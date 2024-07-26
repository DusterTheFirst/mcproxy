use std::{collections::HashMap, net::SocketAddr};

use serde::Deserialize;

use super::util::{Elaborated, Marker};

pub type Config = GenericConfig<Elaborated>;

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct GenericConfig<T: Marker> {
    /// The config for the placeholder server
    pub placeholder_server: PlaceholderServerConfig<T>,
    /// The mapping of servers to their addresses
    pub static_servers: HashMap<String, SocketAddr>,
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
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ProxyConfig {
    /// Address to bind the Minecraft proxy to
    pub listen_address: SocketAddr,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct PlaceholderServerConfig<T: Marker> {
    /// The responses config files
    pub responses: PlaceholderServerResponses<T>,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct PlaceholderServerResponses<T: Marker> {
    /// Response for server when mapping exists but connection failed
    pub offline: Option<T::PointerType>,
    /// Response for server when no mapping exists
    pub no_mapping: Option<T::PointerType>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct UiServerConfig {
    /// Address to bind the HTTP server to
    pub listen_address: SocketAddr,
}

#[cfg(test)]
mod test {
    use crate::{
        config::{schema::GenericConfig, util::Raw},
        proto::response::StatusResponse,
    };

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

    #[test]
    fn config_schema() {
        generate_schema_for::<GenericConfig<Raw>>("config.schema.json");
    }

    #[test]
    fn response_schema() {
        generate_schema_for::<StatusResponse>("response.schema.json");
    }
}
