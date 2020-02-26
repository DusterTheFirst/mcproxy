use crate::proto::packet::{response::Response, Chat};
use async_std::fs;
use async_std::io;
use async_std::net::IpAddr;
use async_std::net::SocketAddr;
use async_std::path::Path;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;

async fn load_toml<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> io::Result<T> {
    Ok(toml::from_str(&fs::read_to_string(path).await?)?)
}

/// Convert the favicon from a URL to the rendered base64 data
async fn load_favicon(response: Response) -> io::Result<Response> {
    Ok(Response {
        favicon: if let Some(favicon) = response.favicon {
            Some(format!(
                "data:image/png;base64,{}",
                base64::encode(&fs::read(favicon).await?)
            ))
        } else {
            None
        },
        ..response
    })
}

#[derive(Deserialize, Debug)]
pub struct Config {
    /// The config for the placeholder server
    pub placeholder_server: PlaceholderServerConfig,
    /// The mapping of servers to their addresses
    pub servers: HashMap<String, SocketAddr>,
    /// Settings for the proxy server
    pub proxy: ProxyConfig,
}

impl Config {
    pub async fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        load_toml(path).await
    }
}

#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    /// The port to bind to
    pub port: u16,
    /// The address to bind to
    pub address: IpAddr,
}

#[derive(Deserialize, Debug)]
pub struct PlaceholderServerConfig {
    /// The message to use when kicking a user from the server
    pub kick_message: Chat,
    /// The responses config files
    pub responses: PlaceholderServerResponsesFiles,
}

pub type PlaceholderServerResponsesFiles = PlaceholderServerResponses<String>;
pub type PlaceholderServerResponsesParsed = PlaceholderServerResponses<Response>;

impl PlaceholderServerResponsesFiles {
    pub async fn load(&self) -> io::Result<PlaceholderServerResponsesParsed> {
        Ok(PlaceholderServerResponsesParsed {
            offline: match &self.offline {
                Some(path) => Some(load_favicon(load_toml(&path).await?).await?),
                None => None,
            },
            no_mapping: match &self.no_mapping {
                Some(path) => Some(load_favicon(load_toml(&path).await?).await?),
                None => None,
            },
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct PlaceholderServerResponses<T> {
    /// Response for server when mapping exists but connection failed
    pub offline: Option<T>,
    /// Response for server when no mapping exists
    pub no_mapping: Option<T>,
}
