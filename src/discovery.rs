use std::{
    fmt::{Debug, Display},
    ops::{Bound, ControlFlow},
    sync::Arc,
};

use dashmap::DashMap;

#[cfg(feature = "discovery-docker")]
mod docker;

#[derive(Debug)]
struct ActiveServer {
    hostnames: Vec<Arc<str>>,
    port: u16,
}

#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum ServerId {
    #[cfg(feature = "discovery-docker")]
    Docker(docker::ContainerId),
}

impl Debug for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            #[cfg(feature = "discovery-docker")]
            Self::Docker(id) => write!(f, "docker:{id}"),
        }
    }
}

enum ServerInsertionError {
    ServerIdExists,
    HostnameExists(HostnameExistsError),
}

impl Display for ServerInsertionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerInsertionError::ServerIdExists => write!(f, "server id already exists"),
            ServerInsertionError::HostnameExists(error) => Display::fmt(error, f),
        }
    }
}

struct HostnameExistsError {
    hostname: Arc<str>,
    server: ActiveServer,
}
impl HostnameExistsError {
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn server(&self) -> &ActiveServer {
        &self.server
    }
}

impl Display for HostnameExistsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hostname {} already mapped", self.hostname)
    }
}

#[derive(Default, Debug)]
struct DiscoveredServers {
    active_servers: DashMap<ServerId, ActiveServer>,

    hostname_index: DashMap<Arc<str>, ServerId>,
}

impl DiscoveredServers {
    fn insert(&self, id: ServerId, server: ActiveServer) -> Result<(), ServerInsertionError> {
        let vacant_entry = match self.active_servers.entry(id) {
            dashmap::Entry::Occupied(_) => return Err(ServerInsertionError::ServerIdExists),
            dashmap::Entry::Vacant(vacant) => vacant,
        };

        let add_hostnames = || {
            for (index, hostname) in server.hostnames.iter().cloned().enumerate() {
                match self.hostname_index.entry(hostname) {
                    dashmap::Entry::Occupied(_) => {
                        return ControlFlow::Break(index);
                    }
                    dashmap::Entry::Vacant(empty) => empty.insert(id),
                };
            }
            ControlFlow::Continue(())
        };

        if let ControlFlow::Break(conflict_index) = add_hostnames() {
            // Undo addition when error encountered
            self.drop_index(&server, Some(conflict_index));

            return Err(ServerInsertionError::HostnameExists(HostnameExistsError {
                hostname: server.hostnames[conflict_index].clone(),
                server,
            }));
        }

        vacant_entry.insert(server);

        Ok(())
    }

    fn remove(&self, id: ServerId) -> Option<ActiveServer> {
        let (id, server) = self.active_servers.remove(&id)?;

        self.drop_index(&server, None);

        Some(server)
    }

    fn drop_index(&self, server: &ActiveServer, until: Option<usize>) {
        let range = match until {
            Some(until) => (Bound::Unbounded, Bound::Excluded(until)),
            None => (Bound::Unbounded, Bound::Unbounded),
        };

        for hostname in &server.hostnames[range] {
            self.hostname_index.remove(hostname);
        }
    }
}

pub async fn begin() {
    #[cfg(feature = "discovery-docker")]
    tokio::task::spawn(async {
        tracing::info!("docker discovery enabled");
        match docker::docker().await {
            Ok(()) => tracing::warn!("docker discovery exited early"),
            Err(error) => tracing::error!(%error, "docker discovery encountered an error"),
        }
    });
}
