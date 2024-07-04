use std::{ops::ControlFlow, sync::Arc, time::Duration};

use tokio::{io, net::TcpStream, time::timeout};
use tracing::{debug, error, field, trace, trace_span, warn, Instrument, Span};
use tracing_error::TracedError;

use crate::{config::schema::Config, proto::{
    io::{
        read_handshake,
        response::{login_response, ping_response},
        write_packet,
    },
    Handshake, NextState,
}};

const PING_TIMEOUT: Duration = Duration::from_millis(300);

macro_rules! timeout_break {
    ($timeout:ident, $response:expr) => {
        match timeout($timeout, $response).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                debug!("timeout exceeded");
                return Ok(ControlFlow::Break(()));
            }
        }
    };
}

#[tracing::instrument(name="routing", skip_all, fields(connection=connection_id, address=field::Empty, next_state=field::Empty, upstream=field::Empty))]
pub async fn handle_connection(
    connection_id: u16,
    config: Arc<Config>,
    #[cfg(feature = "discovery")] discovered_servers: Arc<crate::discovery::DiscoveredServers>,
    client_stream: &mut TcpStream,
) -> Result<ControlFlow<(), (TcpStream, Handshake)>, TracedError<io::Error>> {
    // TODO: Handle legacy ping
    trace!("new connection");

    // First, the client sends a Handshake packet with its state set to 1.
    let (handshake, handshake_packet) = timeout_break!(PING_TIMEOUT, read_handshake(client_stream));
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
    let upstream = config.static_servers.get(&handshake.address).copied();

    #[cfg(feature = "discovery")]
    let upstream = upstream.or_else(|| {
        discovered_servers
            .get_by_hostname(&handshake.address)
            .map(|server| server.upstream())
    });

    let upstream = match upstream {
        Some(a) => a,
        None => {
            warn!("unknown address");

            match handshake.next_state {
                NextState::Ping => {
                    timeout_break!(
                        PING_TIMEOUT,
                        ping_response(
                            client_stream,
                            config.placeholder_server.responses.no_mapping.as_ref()
                        )
                    );
                    return Ok(ControlFlow::Break(()));
                }
                NextState::Login => {
                    timeout_break!(
                        PING_TIMEOUT,
                        login_response(
                            client_stream,
                            config
                                .placeholder_server
                                .responses
                                .no_mapping
                                .as_ref()
                                .map(|res| &res.description)
                        )
                    );
                    return Ok(ControlFlow::Break(()));
                }
                NextState::Transfer => {
                    error!("unimplemented");
                    return Ok(ControlFlow::Break(()));
                }
                NextState::Unknown(state) => {
                    warn!(state, "unknown next_state");
                    return Ok(ControlFlow::Break(()));
                }
            }
        }
    };
    Span::current().record("upstream", upstream.to_string());

    let mut server_stream = match TcpStream::connect(upstream)
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
                NextState::Ping => {
                    timeout_break!(
                        PING_TIMEOUT,
                        ping_response(
                            client_stream,
                            config.placeholder_server.responses.offline.as_ref()
                        )
                    );
                    return Ok(ControlFlow::Break(()));
                }
                NextState::Login => {
                    timeout_break!(
                        PING_TIMEOUT,
                        login_response(
                            client_stream,
                            config
                                .placeholder_server
                                .responses
                                .offline
                                .as_ref()
                                .map(|res| &res.description)
                        )
                    );
                    return Ok(ControlFlow::Break(()));
                }
                NextState::Transfer => {
                    error!("unimplemented");
                    return Ok(ControlFlow::Break(()));
                }
                NextState::Unknown(state) => {
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