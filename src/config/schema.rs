use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};

use serde::Deserialize;

use crate::proto::TextComponent;

use super::util::{Elaborated, Marker};

pub type Config = GenericConfig<Elaborated>;

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct GenericConfig<T: Marker> {
    /// The config for the placeholder server
    pub placeholder_server: PlaceholderServerConfig<T>,
    /// The mapping of servers to their addresses
    pub servers: HashMap<String, SocketAddr>,
    /// Settings for the proxy server
    pub proxy: ProxyConfig,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProxyConfig {
    /// The port to bind to
    pub port: u16,
    /// The address to bind to
    pub address: IpAddr,
}

#[derive(Deserialize, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PlaceholderServerConfig<T: Marker> {
    /// The message to use when kicking a user from the server
    pub kick_message: TextComponent, // TODO: remove?
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
