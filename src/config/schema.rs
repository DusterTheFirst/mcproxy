use std::{collections::HashMap, fmt::Display, net::SocketAddr, sync::Arc};

use serde::{de::Visitor, Deserialize};
use tokio::net::ToSocketAddrs;

use super::util::{Elaborated, Marker};

pub type Config = GenericConfig<Elaborated>;

#[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct Hostname(Arc<str>);

impl From<String> for Hostname {
    fn from(value: String) -> Self {
        Hostname(Arc::from(value))
    }
}

impl AsRef<str> for Hostname {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Upstream {
    pub host: Arc<str>,
    pub port: u16,
}

impl From<SocketAddr> for Upstream {
    fn from(value: SocketAddr) -> Self {
        Upstream {
            host: Arc::from(value.ip().to_string()),
            port: value.port(),
        }
    }
}

impl Display for Upstream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

#[cfg(feature = "metrics")]
impl prometheus_client::encoding::EncodeLabelSet for Upstream {
    fn encode(
        &self,
        mut encoder: prometheus_client::encoding::LabelSetEncoder,
    ) -> Result<(), std::fmt::Error> {
        let mut label_encoder = encoder.encode_label();
        let mut label_key_encoder = label_encoder.encode_label_key()?;
        prometheus_client::encoding::EncodeLabelKey::encode(&"host", &mut label_key_encoder)?;

        let mut label_value_encoder = label_key_encoder.encode_label_value()?;
        prometheus_client::encoding::EncodeLabelValue::encode(
            &&*self.host,
            &mut label_value_encoder,
        )?;

        label_value_encoder.finish()?;

        let mut label_encoder = encoder.encode_label();
        let mut label_key_encoder = label_encoder.encode_label_key()?;
        prometheus_client::encoding::EncodeLabelKey::encode(&"port", &mut label_key_encoder)?;

        let mut label_value_encoder = label_key_encoder.encode_label_value()?;
        prometheus_client::encoding::EncodeLabelValue::encode(
            &self.port,
            &mut label_value_encoder,
        )?;

        label_value_encoder.finish()?;

        Ok(())
    }
}

impl Upstream {
    pub fn addr(&self) -> impl ToSocketAddrs + '_ {
        (&*self.host, self.port)
    }
}

impl<'d> Deserialize<'d> for Upstream {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'d>,
    {
        struct UpstreamVisitor;

        impl<'d> Visitor<'d> for UpstreamVisitor {
            type Value = Upstream;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a string like <host_name>:<port> pair where <port> is a u16 value"
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let (host, port) = v
                    .split_once(':')
                    .ok_or(E::invalid_value(serde::de::Unexpected::Str(v), &self))?;

                let port = port.parse().map_err(|_| {
                    E::invalid_value(serde::de::Unexpected::Str(port), &"a u16 value")
                })?;

                Ok(Upstream {
                    host: Arc::from(host),
                    port,
                })
            }
        }

        deserializer.deserialize_str(UpstreamVisitor)
    }
}

#[cfg(test)]
impl schemars::JsonSchema for Upstream {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Upstream".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        // Include the module, in case a type with the same name is in another module/crate
        concat!(module_path!(), "::Upstream").into()
    }

    fn json_schema(gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        gen.root_schema_for::<&str>()
    }
}

#[derive(Deserialize, Debug)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct GenericConfig<T: Marker> {
    /// The config for the placeholder server
    pub placeholder_server: PlaceholderServerConfig<T>,
    /// The mapping of servers to their addresses
    pub static_servers: HashMap<Hostname, Upstream>,
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
