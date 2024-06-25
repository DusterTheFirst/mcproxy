use proto::{
    io::{
        read_handshake, read_packet, read_ping_request, write_packet, write_pong_response,
        write_status_response,
    },
    response::Response,
    Handshake,
};
use std::{collections::HashMap, net::SocketAddr, ops::ControlFlow, sync::Arc, time::Duration};
use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task,
    time::timeout,
};
use trace::init_tracing_subscriber;
use tracing::{debug, error, field, info, trace, trace_span, warn, Instrument, Span};
use tracing_error::{InstrumentResult, TracedError};

use crate::config::{Config, PlaceholderServerResponsesParsed};
use crate::proxy_server::ProxyServer;

pub mod config;
pub mod proto;
pub mod proxy_server;
pub mod trace;

#[tokio::main]
async fn main() -> io::Result<()> {
    init_tracing_subscriber();

    info!("proxy starting");

    // TODO: config file + cmd line opts
    let config: Config = Config::load("./example/config.toml").await?;

    let address_map = Arc::new(config.servers);
    let server_responses: Arc<PlaceholderServerResponsesParsed> =
        Arc::new(config.placeholder_server.responses.load().await?);

    let listener = TcpListener::bind(SocketAddr::new(config.proxy.address, config.proxy.port))
        .await
        .expect("Unable to bind to socket");

    info!(
        port = config.proxy.port,
        address = %config.proxy.address,
        "proxy server listening",
    );

    // Accept connections as they come in
    loop {
        let stream = listener.accept().await;

        match stream {
            Ok((mut client_stream, address)) => {
                // Clone pointers to the address map and server responses
                let address_map = address_map.clone();
                let server_responses = server_responses.clone();

                // Get the connection id
                let connection_id = client_stream.peer_addr().unwrap().port();

                // Fork off the connection handling
                let task = async move {
                    // Handle the connection
                    match handle_connection(
                        connection_id,
                        address_map,
                        server_responses,
                        &mut client_stream,
                    )
                    .await
                    {
                        Ok(ControlFlow::Continue((server_stream, handshake))) => {
                            // Spin up constant proxy until the connection is complete
                            ProxyServer::new(server_stream, client_stream)
                                .start()
                                .instrument(trace_span!(
                                    "proxy",
                                    connection=connection_id,
                                    address = handshake.address,
                                    next_state = %handshake.next_state
                                ))
                                .await;
                        }
                        Ok(ControlFlow::Break(())) => {}
                        Err(e) => {
                            error!("Error in handling connection: {}", e);
                        }
                    };
                }
                .instrument(trace_span!("connection"));

                #[cfg(feature = "tokio-console")]
                task::Builder::new()
                    .name(&connection_id.to_string())
                    .spawn(task)
                    .unwrap();

                #[cfg(not(feature = "tokio-console"))]
                task::spawn(task);
            }
            Err(e) => error!("Error connecting to client: {}", e),
        }
    }
}

const PING_TIMEOUT: Duration = Duration::from_millis(300);

macro_rules! timeout_break {
    ($timeout:ident, $response:expr) => {
        match timeout($timeout, $response).await {
            Ok(Ok(())) => return Ok(ControlFlow::Break(())),
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                debug!("timeout exceeded");
                return Ok(ControlFlow::Break(()));
            }
        }
    };
}

#[tracing::instrument(name="routing", skip_all, fields(connection=connection_id, address=field::Empty, next_state=field::Empty))]
async fn handle_connection(
    connection_id: u16,
    address_map: Arc<HashMap<String, SocketAddr>>,
    server_responses: Arc<PlaceholderServerResponsesParsed>,
    client_stream: &mut TcpStream,
) -> Result<ControlFlow<(), (TcpStream, Handshake)>, TracedError<io::Error>> {
    // TODO: Handle legacy ping
    trace!("new connection");

    // First, the client sends a Handshake packet with its state set to 1.
    let (handshake, handshake_packet) = read_handshake(client_stream).await?;
    debug!(
        address = handshake.address,
        port = handshake.port,
        protocol_version = handshake.protocol_version,
        next_state = ?handshake.next_state,
        "handshake received"
    );
    Span::current().record("address", &handshake.address);
    Span::current().record("next_state", handshake.next_state.to_string());

    // Handle mapping
    let address = match address_map.get(&handshake.address) {
        Some(a) => a,
        None => {
            warn!("unknown address");

            match handshake.next_state {
                proto::NextState::Ping => {
                    timeout_break!(
                        PING_TIMEOUT,
                        ping_response(client_stream, server_responses.no_mapping.as_ref())
                    );
                }
                proto::NextState::Login => todo!(),
                proto::NextState::Transfer => {
                    error!("unimplemented");
                    return Ok(ControlFlow::Break(()));
                }
                proto::NextState::Unknown(state) => {
                    warn!(state, "unknown next_state");
                    return Ok(ControlFlow::Break(()));
                }
            }
        }
    };

    let mut server_stream = match TcpStream::connect(address).await {
        Ok(stream) => stream,
        Err(error) => {
            error!(
                %error,
                "could not connect to upstream"
            );

            match handshake.next_state {
                proto::NextState::Ping => {
                    timeout_break!(
                        PING_TIMEOUT,
                        ping_response(client_stream, server_responses.offline.as_ref())
                    );
                }
                proto::NextState::Login => todo!(),
                proto::NextState::Transfer => {
                    error!("unimplemented");
                    return Ok(ControlFlow::Break(()));
                }
                proto::NextState::Unknown(state) => {
                    warn!(state, "unknown next_state");
                    return Ok(ControlFlow::Break(()));
                }
            }
        }
    };
    trace!("connected to upstream");

    // Forward the handshake to the upstream
    write_packet(
        &mut server_stream,
        handshake_packet.id,
        &handshake_packet.data,
    )
    .await?;

    trace!("passing upstream to proxy");

    Ok(ControlFlow::Continue((server_stream, handshake)))
}

async fn ping_response(
    client_stream: &mut TcpStream,
    response: Option<&Response>,
) -> Result<(), TracedError<io::Error>> {
    // The client follows up with a Status Request packet. This packet has no fields. The client is also able to skip this part entirely and send a Ping Request instead.
    read_packet(client_stream).await?;

    if let Some(response) = response {
        // The server should respond with a Status Response packet.
        write_status_response(client_stream, response).await?;
    }

    // If the process is continued, the client will now send a Ping Request packet containing some payload which is not important.
    let packet = read_ping_request(client_stream).await?;
    // The server will respond with the Pong Response packet and then close the connection.
    write_pong_response(client_stream, packet).await?;

    InstrumentResult::in_current_span(client_stream.shutdown().await)?;

    Ok(())
}
