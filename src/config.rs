use crate::proto::packet::{response::Response, TextComponent};
use base64::Engine;
use serde::{de::DeserializeOwned, Deserialize};
use std::{
    collections::HashMap,
    fmt::Debug,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use tokio::{
    fs,
    io::{self, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener,
    },
    task,
};
use tracing::{error, info};

async fn load_toml<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> io::Result<T> {
    toml::from_str(&fs::read_to_string(path).await?)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}

/// Convert the favicon from a URL to the rendered base64 data
async fn load_favicon(response: Response) -> io::Result<Response> {
    Ok(Response {
        favicon: if let Some(favicon) = response.favicon {
            Some(format!(
                "data:image/png;base64,{}",
                base64::prelude::BASE64_STANDARD.encode(&fs::read(favicon).await?)
            ))
        } else {
            None
        },
        ..response
    })
}

pub type Config = GenericConfig<Elaborated>;

#[derive(Deserialize, Debug)]
pub struct GenericConfig<T: Marker> {
    /// The config for the placeholder server
    pub placeholder_server: PlaceholderServerConfig<T>,
    /// The mapping of servers to their addresses
    pub servers: HashMap<String, SocketAddr>,
    /// Settings for the proxy server
    pub proxy: ProxyConfig,
}

impl Config {
    pub async fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let raw = load_toml::<GenericConfig<Raw>, _>(path).await?;

        Ok(Config {
            placeholder_server: PlaceholderServerConfig {
                kick_message: raw.placeholder_server.kick_message,
                responses: PlaceholderServerResponses {
                    offline: match &raw.placeholder_server.responses.offline {
                        Some(path) => Some(load_favicon(load_toml(&path).await?).await?),
                        None => None,
                    },
                    no_mapping: match &raw.placeholder_server.responses.no_mapping {
                        Some(path) => Some(load_favicon(load_toml(&path).await?).await?),
                        None => None,
                    },
                },
            },
            servers: raw.servers,
            proxy: raw.proxy,
        })
    }

    pub async fn load_and_watch(
        path: PathBuf,
    ) -> Result<tokio::sync::watch::Receiver<Arc<Config>>, io::Error> {
        let address = SocketAddr::from_str("127.0.0.1:9876").unwrap();
        let socket = TcpListener::bind(address).await?;
        info!(%address, "UI running");

        let (sender, receiver) = tokio::sync::watch::channel(Arc::new(Config::load(&path).await?));

        task::spawn(async move {
            loop {
                match socket.accept().await {
                    Ok((mut stream, address)) => {
                        let (reader, writer) = stream.split();

                        let reader = BufReader::new(reader);
                        let writer = BufWriter::new(writer);

                        match handle_http(reader, writer, &path).await {
                            Ok(None) => {}
                            Ok(Some(config)) => {
                                sender.send_replace(config);
                            }
                            Err(error) => error!(%error, "encountered error handling http request"),
                        }
                    }
                    Err(error) => error!(%error, "failed to process http request"),
                }
            }
        });

        Ok(receiver)
    }
}

async fn handle_http(
    mut reader: BufReader<ReadHalf<'_>>,
    mut writer: BufWriter<WriteHalf<'_>>,
    path: impl AsRef<Path>,
) -> io::Result<Option<Arc<Config>>> {
    const ENDPOINT: &str = "POST /-/reload ";

    let mut request = [0u8; ENDPOINT.len()];
    reader.read_exact(&mut request).await?;

    let request = String::from_utf8_lossy(&request);

    if request.starts_with(ENDPOINT) {
        match Config::load(path).await {
            Ok(new_config) => {
                info!("configuration reloaded");

                writer
                    .write_all(
                        "HTTP/1.1 200 OK\r\n\r\nConfiguration reloaded successfully".as_bytes(),
                    )
                    .await?;

                writer.flush().await?;

                return Ok(Some(Arc::new(new_config)));
            }
            Err(error) => {
                writer
                    .write_all(
                        "HTTP/1.1 500 Internal Server Error\r\n\r\nFailed to reload configuration"
                            .as_bytes(),
                    )
                    .await?;
                writer.write_all(error.to_string().as_bytes()).await?;
            }
        }
    } else {
        writer
            .write_all("HTTP/1.1 404 Not Found\r\n\r\n".as_bytes())
            .await?;
    }

    writer.flush().await?;

    Ok(None)
}

#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    /// The port to bind to
    pub port: u16,
    /// The address to bind to
    pub address: IpAddr,
}

#[derive(Deserialize, Debug)]
pub struct PlaceholderServerConfig<T: Marker> {
    /// The message to use when kicking a user from the server
    pub kick_message: TextComponent, // TODO: remove?
    /// The responses config files
    pub responses: PlaceholderServerResponses<T>,
}

#[derive(Deserialize, Debug)]
pub struct PlaceholderServerResponses<T: Marker> {
    /// Response for server when mapping exists but connection failed
    pub offline: Option<T::PointerType>,
    /// Response for server when no mapping exists
    pub no_mapping: Option<T::PointerType>,
}

mod private {
    pub trait Sealed {}
}

pub trait Marker: private::Sealed {
    type PointerType: DeserializeOwned + Debug;
}

#[derive(Deserialize)]
pub struct Raw {}
impl private::Sealed for Raw {}
impl Marker for Raw {
    type PointerType = PathBuf;
}

#[derive(Deserialize)]
pub struct Elaborated {}
impl private::Sealed for Elaborated {}
impl Marker for Elaborated {
    type PointerType = Response;
}
