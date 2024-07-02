use proto::{
    io::{
        read_handshake, read_packet, read_ping_request, write_packet, write_pong_response,
        write_status_response,
    },
    response::StatusResponse,
    string, Handshake,
};
use std::{ops::ControlFlow, sync::Arc, time::Duration};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task,
    time::timeout,
};
use trace::init_tracing_subscriber;
use tracing::{debug, error, field, info, trace, trace_span, warn, Instrument, Span};
use tracing_error::{InstrumentResult, TracedError};

use crate::config::Config;
use crate::proxy_server::ProxyServer;

pub mod config;
pub mod proto;
pub mod proxy_server;
pub mod trace;

#[cfg(feature = "discovery")]
pub mod discovery;

// TODO: FIXME: make better
include!(concat!(env!("OUT_DIR"), "/features.rs"));

#[tokio::main]
async fn main() -> eyre::Result<()> {
    init_tracing_subscriber();

    info!(features=?ENABLED_FEATURES, "proxy starting");

    // TODO: command line options
    let config = config::load_and_watch("./example/config.toml".into()).await?;
    let current_config = config.borrow().clone();

    #[cfg(feature = "discovery")]
    let discovered_servers = discovery::begin().await;

    let listener = TcpListener::bind(current_config.proxy.listen_address)
        .await
        .expect("Unable to bind to socket");

    info!(
        listen_address = %current_config.proxy.listen_address,
        "proxy server listening",
    );

    drop(current_config);

    // Accept connections as they come in
    loop {
        let stream = listener.accept().await;

        match stream {
            Ok((mut client_stream, address)) => {
                // Clone pointers to the address map and server responses
                let config = config.borrow().clone();
                let discovered_servers = discovered_servers.clone(); // TODO:

                // Get the connection id
                let connection_id = client_stream.peer_addr().unwrap().port();

                // Fork off the connection handling
                let task = async move {
                    // Handle the connection
                    match handle_connection(connection_id, config, &mut client_stream).await {
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
    config: Arc<Config>,
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
    let address = match config.static_servers.get(&handshake.address) {
        Some(a) => a,
        None => {
            warn!("unknown address");

            match handshake.next_state {
                proto::NextState::Ping => {
                    timeout_break!(
                        PING_TIMEOUT,
                        ping_response(
                            client_stream,
                            config.placeholder_server.responses.no_mapping.as_ref()
                        )
                    );
                }
                proto::NextState::Login => {
                    timeout_break!(
                        PING_TIMEOUT,
                        login_response(
                            client_stream,
                            config.placeholder_server.responses.no_mapping.as_ref()
                        )
                    );
                }
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

    let mut server_stream = match TcpStream::connect(address)
        .instrument(trace_span!("connect_upstream"))
        .await
    {
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
                        ping_response(
                            client_stream,
                            config.placeholder_server.responses.offline.as_ref()
                        )
                    );
                }
                proto::NextState::Login => {
                    timeout_break!(
                        PING_TIMEOUT,
                        login_response(
                            client_stream,
                            config.placeholder_server.responses.offline.as_ref()
                        )
                    );
                }
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

#[tracing::instrument(skip_all, err)]
async fn ping_response(
    client_stream: &mut TcpStream,
    response: Option<&StatusResponse>,
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

#[tracing::instrument(skip_all, err)]
async fn login_response(
    client_stream: &mut TcpStream,
    response: Option<&StatusResponse>,
) -> Result<(), TracedError<io::Error>> {
    // TODO: put this in a struct
    let packet = read_packet(client_stream).await?;
    assert_eq!(packet.id, 0x00);

    let mut data_buf = packet.data.as_slice();
    // TODO: no need for these to be async
    let name = string::read(&mut data_buf).await?;
    let uuid = data_buf.read_u128().await?;

    println!("{name}: {uuid:x?}");

    if let Some(response) = response {
        // TODO: I can totally mechanize the construction of packets, maybe look into that?
        write_packet(
            client_stream,
            0x00,
            &string::write(&serde_json::to_string(&response.description).unwrap()),
        )
        .await?;
    }

    Ok(())
}
