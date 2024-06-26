use std::{collections::HashMap, net::SocketAddr, path::PathBuf};

use serde::Deserialize;

use super::util::{Elaborated, Marker};

pub type Config = GenericConfig<Elaborated>;

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct GenericConfig<T: Marker> {
    /// The config for the placeholder server
    pub placeholder_server: PlaceholderServerConfig<T>,
    /// The mapping of servers to their addresses
    pub static_servers: HashMap<String, SocketAddr>,
    /// Server discovery configuration
    pub discovery: Option<ServerDiscoveryConfig>,
    /// Setting for the UI Server
    ///
    /// Can not be live-reloaded
    pub ui: Option<UiServerConfig>,
    /// Settings for the proxy server
    ///
    /// Can not be live-reloaded
    pub proxy: ProxyConfig,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProxyConfig {
    /// Address to bind the Minecraft proxy to
    pub listen_address: SocketAddr,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PlaceholderServerConfig<T: Marker> {
    /// The responses config files
    pub responses: PlaceholderServerResponses<T>,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PlaceholderServerResponses<T: Marker> {
    /// Response for server when mapping exists but connection failed
    pub offline: Option<T::PointerType>,
    /// Response for server when no mapping exists
    pub no_mapping: Option<T::PointerType>,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct UiServerConfig {
    /// Address to bind the HTTP server to
    pub listen_address: SocketAddr,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ServerDiscoveryConfig {
    /// Configuration for docker service discovery
    docker: Option<DockerServerDiscoveryConfig>,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct DockerServerDiscoveryConfig {
    /// Path to the docker socket
    socket: PathBuf,
}

#[cfg(test)]
mod test {
    use crate::{
        config::{schema::GenericConfig, util::Raw},
        proto::response::StatusResponse,
    };

    #[cfg(feature = "schemars")]
    fn generate_schema_for<T: ?Sized + schemars::JsonSchema>(filename: &str) {
        let file = [env!("CARGO_MANIFEST_DIR"), "target", "schema", filename]
            .into_iter()
            .collect::<std::path::PathBuf>();

        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            file,
            serde_json::to_string_pretty(
                &schemars::SchemaGenerator::new(schemars::gen::SchemaSettings::draft07())
                    .into_root_schema_for::<T>(),
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[cfg(not(feature = "schemars"))]
    fn generate_schema_for<T>(_: &str) {
        unreachable!()
    }

    #[test]
    #[cfg_attr(not(feature = "schemars"), ignore = "requires feature `schemars`")]
    fn config_schema() {
        generate_schema_for::<GenericConfig<Raw>>("config.schema.json");
    }

    #[test]
    #[cfg_attr(not(feature = "schemars"), ignore = "requires feature `schemars`")]
    fn response_schema() {
        generate_schema_for::<StatusResponse>("response.schema.json");
    }
}
