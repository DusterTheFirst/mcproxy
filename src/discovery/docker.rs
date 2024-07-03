use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use bollard::{
    container::{InspectContainerOptions, ListContainersOptions},
    secret::{
        ContainerInspectResponse, ContainerSummary, ContainerSummaryNetworkSettings, EventActor,
        EventMessage, EventMessageScopeEnum, EventMessageTypeEnum, NetworkSettings,
    },
    system::EventsOptions,
};
use eyre::Context;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

use crate::discovery::{ActiveServer, DiscoveredServers, ServerId, ServerInsertionError};

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ContainerId([u8; 32]);

impl Debug for ContainerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for ContainerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

// How to handle multiple replicas with the same hostname
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplicaBehavior {
    // Prepend a subdomain with the index of the replica: [index].[hostname]
    IndexSubdomain,
}

struct InvalidReplicaBehaviorError;
impl Display for InvalidReplicaBehaviorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unknown replica behavior. known values include: index-subdomain"
        )
    }
}

impl FromStr for ReplicaBehavior {
    type Err = InvalidReplicaBehaviorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "index-subdomain" => Ok(ReplicaBehavior::IndexSubdomain),
            _ => Err(InvalidReplicaBehaviorError),
        }
    }
}

impl FromStr for ContainerId {
    type Err = eyre::Report;

    fn from_str(hex: &str) -> Result<Self, Self::Err> {
        let mut hex_bytes = hex
            .as_bytes()
            .iter()
            .filter_map(|b| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            })
            .fuse();

        let mut array = [0; 32];

        for byte in array.iter_mut() {
            if let (Some(h), Some(l)) = (hex_bytes.next(), hex_bytes.next()) {
                *byte = h << 4 | l
            } else {
                return Err(eyre::eyre!("too few nibbles"));
            }
        }

        if hex_bytes.next().is_some() {
            return Err(eyre::eyre!("too many nibbles"));
        }

        Ok(Self(array))
    }
}

fn extract_label<T>(labels: &HashMap<String, String>, key: &str) -> Option<T>
where
    T: FromStr,
    <T as FromStr>::Err: Display,
{
    labels.get(key).and_then(|value| match T::from_str(value) {
        Ok(value) => Some(value),
        Err(error) => {
            error!(
                label_name = key,
                label_value = value,
                error = %error,
                "invalid value provided in container labels"
            );

            None
        }
    })
}

fn gather_server_information(
    ip: IpAddr,
    mut labels: HashMap<String, String>,
) -> Option<ActiveServer> {
    let replica_behavior = extract_label(&labels, "mcproxy.replica_behavior");

    if let Some(hostname) = labels.remove("mcproxy") {
        let hostname = match replica_behavior.zip(labels.get("com.docker.compose.container-number"))
        {
            Some((ReplicaBehavior::IndexSubdomain, replica)) => {
                Arc::from(format!("{replica}.{hostname}"))
            }
            None => Arc::from(hostname),
        };

        let port = extract_label(&labels, "mcproxy.port");

        Some(ActiveServer {
            hostnames: vec![hostname],
            upstream: SocketAddr::new(ip, port.unwrap_or(25565)),
        })
    } else {
        warn!(
            label_name = "mcproxy",
            "container listing included container with no mcproxy label"
        );

        None
    }
}

pub async fn docker(discovered_servers: Arc<DiscoveredServers>) -> Result<(), eyre::Report> {
    tracing::info!("docker discovery started");

    let docker =
        bollard::Docker::connect_with_defaults().wrap_err("failed to connect to docker socket")?;

    let current_containers = match docker
        .list_containers(Some(ListContainersOptions {
            filters: HashMap::from_iter([("label", vec!["mcproxy"])]),
            ..Default::default()
        }))
        .await
    {
        Ok(containers) => containers,
        Err(error) => {
            warn!(%error, "could not list running containers");

            vec![]
        }
    };

    // dbg!(&current_containers);

    let current_active_servers =
            current_containers
                .into_iter()
                .filter_map(|container| match container {
                    ContainerSummary {
                        id: Some(id),
                        ports: Some(ports),
                        labels: Some(labels),
                        network_settings: Some(ContainerSummaryNetworkSettings { networks: Some(networks) }),
                        ..
                    } => {
                        // FIXME: smarter network selection.... Use docker networks, also error handling is a mess here
                        let ip = networks.iter().next().and_then(|(network_name, endpoint_settings)| {
                            endpoint_settings.ip_address.as_ref()
                        }).and_then(|ip| IpAddr::from_str(ip).inspect_err(|err| error!(
                            error = %err,
                            "invalid ip address returned from docker daemon"
                        )).ok())?;

                        // dbg!(ports);
                        // dbg!(labels.get("mcproxy.port"));
                        let container_id = ContainerId::from_str(&id)
                        .inspect_err(|error| warn!(%error, "container id malformed"))
                        .ok()?;

                        gather_server_information(ip, labels).map(|server| (container_id, server))
                    }
                    container => {
                        warn!(?container.id, container.ports.exists = container.ports.is_some(), container.labels.exists = container.labels.is_some(), "container listing contained incomplete information");
                        None
                    }
                });

    for (container, server) in current_active_servers {
        match discovered_servers.insert(ServerId::Docker(container), server) {
            Err(ServerInsertionError::ServerIdExists) => {
                error!("server id already exists in mapping")
            }
            Err(ServerInsertionError::HostnameExists(error)) => {
                warn!(
                    hostname = error.hostname(),
                    %container,
                    "server with hostname exists already"
                );
            }
            Ok(()) => (),
        };
    }

    // dbg!(&discovered_servers);

    info!(
        count = discovered_servers.len(),
        "discovered running docker servers"
    );

    let mut events = docker.events(Some(EventsOptions::<&str> {
        filters: HashMap::from_iter([
            ("type", vec!["container"]),
            ("event", vec!["start", "stop"]),
            ("label", vec!["mcproxy"]),
        ]),
        ..Default::default()
    }));

    while let Some(event) = events.next().await {
        match event {
            Ok(EventMessage {
                typ: Some(EventMessageTypeEnum::CONTAINER),
                action: Some(action),
                actor:
                    Some(EventActor {
                        id: Some(id),
                        attributes: Some(attributes),
                    }),
                scope: Some(EventMessageScopeEnum::LOCAL),
                time,
                time_nano,
            }) => {
                info!(?action, ?time, ?time_nano, ?id);

                // dbg!(attributes.get("name"));
                // dbg!(attributes.get("com.docker.compose.container-number"));
                // dbg!(attributes.get("com.docker.compose.service"));
                // dbg!(attributes.get("com.docker.compose.project"));

                let id = match ContainerId::from_str(&id) {
                    Ok(id) => id,
                    Err(error) => {
                        warn!(%error, "container id malformed");

                        continue;
                    }
                };

                match action.as_str() {
                    "start" => {
                        let ip =
                            match docker
                                .inspect_container(
                                    &id.to_string(),
                                    Some(InspectContainerOptions {
                                        ..Default::default()
                                    }),
                                )
                                .await
                            {
                                Ok(ContainerInspectResponse {
                                    network_settings:
                                        Some(NetworkSettings {
                                            networks: Some(networks),
                                            ..
                                        }),
                                    ..
                                }) => {
                                    // FIXME: smarter network selection.... Use docker networks, also error handling is a mess here
                                    networks
                                        .iter()
                                        .next()
                                        .and_then(|(network_name, endpoint_settings)| {
                                            endpoint_settings.ip_address.as_ref()
                                        })
                                        .and_then(|ip| {
                                            IpAddr::from_str(ip).inspect_err(|err| error!(
                                                error = %err,
                                                "invalid ip address returned from docker daemon"
                                            )).ok()
                                        })
                                }
                                Ok(message) => {
                                    warn!(%id, ?message, "incomplete response from docker daemon");
                                    continue;
                                }
                                Err(error) => {
                                    warn!(%id, %error, "failed to inspect container");
                                    continue;
                                }
                            };

                        let ip = match ip {
                            Some(ip) => ip,
                            None => {
                                warn!("failed to determine container ip address");
                                continue;
                            }
                        };

                        if let Some(server) = gather_server_information(ip, attributes) {
                            debug!(%id, "inserting discovered server mapping");
                            if let Err(error) =
                                discovered_servers.insert(ServerId::Docker(id), server)
                            {
                                error!(%error, "failed to record discovered server");
                                continue;
                            }
                        } else {
                            warn!("container metadata did not exist");
                            continue;
                        }
                    }
                    "stop" => {
                        debug!(%id, "removing discovered server mapping");
                        discovered_servers.remove(ServerId::Docker(id));
                    }
                    _ => warn!(action, "unknown action received"),
                }

                // dbg!(&discovered_servers);
            }
            Ok(message) => {
                warn!(?message, "incomplete response from docker daemon");
            }
            Err(error) => error!(%error, "encountered error reading from docker daemon"),
        }
    }

    Ok(())
}
