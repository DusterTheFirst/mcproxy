use base64::Engine;
use schema::{GenericConfig, PlaceholderServerConfig, PlaceholderServerResponses};
use serde::de::DeserializeOwned;
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs,
    io::{self, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener,
    },
    sync::watch::Sender,
    task,
};
use tracing::{debug, error, info, trace, trace_span, Instrument};
use tracing_error::{InstrumentError, TracedError};
use util::Raw;

use crate::proto::packet::response::StatusResponse;

mod schema;
mod util;

pub use schema::Config;

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
async fn load_favicon(response: StatusResponse) -> Result<StatusResponse, TracedError<io::Error>> {
    Ok(StatusResponse {
        favicon: if let Some(favicon) = response.favicon {
            Some(format!(
                "data:image/png;base64,{}",
                base64::prelude::BASE64_STANDARD.encode(
                    &fs::read(favicon)
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
    let raw = load_toml::<GenericConfig<Raw>>(path).await?;

    Ok(Config {
        discovery: raw.discovery,
        ui: raw.ui,
        static_servers: raw.static_servers,
        proxy: raw.proxy,
        placeholder_server: PlaceholderServerConfig {
            responses: PlaceholderServerResponses {
                offline: match &raw.placeholder_server.responses.offline {
                    Some(path) => Some(load_favicon(load_toml(path).await?).await?),
                    None => None,
                },
                no_mapping: match &raw.placeholder_server.responses.no_mapping {
                    Some(path) => Some(load_favicon(load_toml(path).await?).await?),
                    None => None,
                },
            },
        },
    })
}

pub async fn load_and_watch(
    path: PathBuf,
) -> Result<tokio::sync::watch::Receiver<Arc<Config>>, TracedError<io::Error>> {
    let initial_config = Arc::new(self::load(&path).await?);

    let (sender, receiver) = tokio::sync::watch::channel(initial_config.clone());

    if let Some(ref ui) = initial_config.ui {
        let socket = TcpListener::bind(ui.listen_address).await?;
        info!(listen_address = %ui.listen_address, "UI running");

        task::spawn(async move {
            let sender = sender;

            loop {
                match socket.accept().await {
                    Ok((mut stream, address)) => {
                        let (reader, writer) = stream.split();

                        let reader = BufReader::new(reader);
                        let writer = BufWriter::new(writer);

                        match handle_http(reader, writer, &sender, &path).await {
                            Ok(()) => {}
                            Err(error) => error!(%error, "encountered error handling http request"),
                        }
                    }
                    Err(error) => error!(%error, "failed to process http request"),
                }
            }
        });
    }

    Ok(receiver)
}

#[tracing::instrument(skip_all, name = "config::handle_http")]
async fn handle_http(
    mut reader: BufReader<ReadHalf<'_>>,
    mut writer: BufWriter<WriteHalf<'_>>,
    sender: &Sender<Arc<Config>>,
    path: &Path,
) -> io::Result<()> {
    const METHOD: &str = "POST";
    const URL: &str = "/-/reload";
    const ENDPOINT: &str = "POST /-/reload ";

    let mut request = [0u8; ENDPOINT.len()];
    reader.read_exact(&mut request).await?;

    let request = String::from_utf8_lossy(&request);
    let mut request = request.split(' ');
    let (Some(method), Some(url_path)) = (request.next(), request.next()) else {
        writer
            .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nBad Request\n")
            .await?;

        writer.flush().await?;

        return Ok(());
    };

    trace!(url_path, method, "ui request");

    if url_path == URL {
        if method == METHOD {
            match self::load(path).await {
                Ok(new_config) => {
                    debug!("new configuration parsed");
                    sender.send_replace(Arc::new(new_config));
                    info!("new configuration loaded");

                    writer
                        .write_all(b"HTTP/1.1 200 OK\r\n\r\nConfiguration reloaded successfully\n")
                        .await?;

                    writer.flush().await?;
                }
                Err(error) => {
                    writer
                    .write_all(
                        b"HTTP/1.1 500 Internal Server Error\r\n\r\nFailed to reload configuration\n",
                    )
                    .await?;
                    writer.write_all(error.to_string().as_bytes()).await?;
                    writer.write_all(b"\n").await?;

                    let mut errors = Vec::new();
                    let mut source = &error as &(dyn Error + 'static);
                    while let Some(error) = source.source() {
                        errors.push(error.to_string());
                        source = error;
                    }

                    for error in errors {
                        writer.write_all(error.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                }
            }
        } else {
            writer
                .write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\nMethod Not Allowed\n")
                .await?;
        }
    } else {
        writer
            .write_all(b"HTTP/1.1 404 Not Found\r\n\r\nPage Not Found\n")
            .await?;
    }

    writer.flush().await?;

    Ok(())
}
