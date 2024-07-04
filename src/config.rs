use base64::Engine;
use schema::{Config, GenericConfig, PlaceholderServerConfig, PlaceholderServerResponses};
use serde::de::DeserializeOwned;
use std::path::Path;
use tokio::{fs, io};
use tracing::{trace_span, Instrument};
use tracing_error::{InstrumentError, TracedError};
use util::Raw;

use crate::proto::packet::response::StatusResponse;

pub mod schema;
pub mod util;

#[tracing::instrument(name = "config::load_toml")]
async fn load_toml<T: DeserializeOwned>(path: &Path) -> Result<T, TracedError<io::Error>> {
    toml::from_str(
        &fs::read_to_string(path)
            .instrument(trace_span!("fs::read_to_string"))
            .await
            .map_err(InstrumentError::in_current_span)?,
    )
    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    .map_err(InstrumentError::in_current_span)
}

/// Convert the favicon from a URL to the rendered base64 data
#[tracing::instrument(name = "config::load_favicon")]
async fn load_favicon(
    working_directory: &Path,
    response: StatusResponse,
) -> Result<StatusResponse, TracedError<io::Error>> {
    Ok(StatusResponse {
        favicon: if let Some(favicon) = response.favicon {
            Some(format!(
                "data:image/png;base64,{}",
                base64::prelude::BASE64_STANDARD.encode(
                    &fs::read(working_directory.join(favicon))
                        .instrument(trace_span!("fs::read"))
                        .await
                        .map_err(InstrumentError::in_current_span)?
                )
            ))
        } else {
            None
        },
        ..response
    })
}

#[tracing::instrument(name = "config::load")]
pub async fn load(path: &Path) -> Result<Config, TracedError<io::Error>> {
    let current_directory = std::env::current_dir().map_err(InstrumentError::in_current_span)?;
    let config_file = current_directory
        .join(path)
        .canonicalize()
        .map_err(InstrumentError::in_current_span)?;
    let config_directory = config_file
        .parent()
        .expect("at this point, path should have a parent");

    let raw = load_toml::<GenericConfig<Raw>>(&config_file).await?;

    Ok(Config {
        discovery: raw.discovery,
        ui: raw.ui,
        static_servers: raw.static_servers,
        proxy: raw.proxy,
        placeholder_server: PlaceholderServerConfig {
            responses: PlaceholderServerResponses {
                offline: match &raw.placeholder_server.responses.offline {
                    Some(path) => {
                        let config_file = config_directory
                            .join(path)
                            .canonicalize()
                            .map_err(InstrumentError::in_current_span)?;
                        let config_directory =
                            config_file.parent().expect("path should have a parent");

                        Some(load_favicon(config_directory, load_toml(&config_file).await?).await?)
                    }
                    None => None,
                },
                no_mapping: match &raw.placeholder_server.responses.no_mapping {
                    Some(path) => {
                        let config_file = config_directory
                            .join(path)
                            .canonicalize()
                            .map_err(InstrumentError::in_current_span)?;
                        let config_directory =
                            config_file.parent().expect("path should have a parent");

                        Some(load_favicon(config_directory, load_toml(&config_file).await?).await?)
                    }
                    None => None,
                },
            },
        },
    })
}
