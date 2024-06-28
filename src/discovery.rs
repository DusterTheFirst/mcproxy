#[cfg(feature = "discovery-docker")]
pub async fn docker() {
    use std::collections::HashMap;

    use bollard::{
        container::ListContainersOptions,
        secret::{EventActor, EventMessage, EventMessageScopeEnum, EventMessageTypeEnum},
        system::EventsOptions,
    };
    use eyre::Context;
    use tokio::task;
    use tokio_stream::StreamExt;
    use tracing::{error, info, warn};

    task::spawn(async {
        info!("docker discovery enabled");
        match inner().await {
            Ok(()) => warn!("docker discovery exited early"),
            Err(error) => error!(%error, "docker discovery encountered an error"),
        }
    });

    async fn inner() -> Result<(), eyre::Report> {
        let docker = bollard::Docker::connect_with_defaults()
            .wrap_err("failed to connect to docker socket")?;

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

        dbg!(current_containers);

        //  INDEXES
        // Map container_id => metadata
        // let active_servers_by_container = HashMap::new();
        // Map host_name => metadata
        // let active_servers_by_host_name = HashMap::new();

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

                    dbg!(attributes.get("name"));
                    dbg!(attributes.get("com.docker.compose.container-number"));
                    dbg!(attributes.get("com.docker.compose.service"));
                    dbg!(attributes.get("com.docker.compose.project"));
                }
                Ok(message) => {
                    warn!(?message, "incomplete response from docker daemon");
                }
                Err(error) => error!(%error, "encountered error reading from docker daemon"),
            }
        }

        Ok(())
    }
}
