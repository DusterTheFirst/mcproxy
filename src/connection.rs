use std::{net::SocketAddr, ops::ControlFlow, sync::Arc, time::Duration};

use mcproxy_model::Upstream;
use tokio::{io, net::TcpStream, time::timeout};
use tracing::Instrument;
use tracing::{debug, error, field, trace, trace_span, warn, Span};
use tracing_error::TracedError;

use crate::{
    config::schema::Config,
    proto::{
        io::{
            read_handshake,
            response::{login_response, ping_response},
            write_packet,
        },
        Handshake, NextState,
    },
};

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

#[tracing::instrument(name="routing", skip_all, fields(peer=%peer,address=field::Empty, next_state=field::Empty, upstream=field::Empty))]
pub async fn handle_connection(
    peer: SocketAddr,
    config: Arc<Config>,
    client_stream: &mut TcpStream,
    #[cfg(feature = "metrics")] connection_metrics: crate::metrics::ConnectionMetrics,
) -> Result<ControlFlow<(), (TcpStream, Upstream, Handshake)>, TracedError<io::Error>> {
    // TODO: Handle legacy ping
    trace!("new connection");

    #[cfg(feature = "metrics")]
    connection_metrics.client_connections.inc();

    // First, the client sends a Handshake packet with its state set to 1.
    let (handshake, handshake_packet) = timeout_break!(PING_TIMEOUT, read_handshake(client_stream));
    debug!(
        address = handshake.address.as_ref(),
        port = handshake.port,
        protocol_version = handshake.protocol_version,
        next_state = ?handshake.next_state,
        "handshake received"
    );
    Span::current().record("address", handshake.address.as_ref());
    Span::current().record("next_state", handshake.next_state.to_string());

    #[cfg(feature = "metrics")]
    connection_metrics.client_handshakes_received.inc();

    // Handle mapping
    let upstream = config.static_servers.get(&handshake.address).cloned();

    let upstream = match upstream {
        Some(a) => a,
        None => {
            warn!("unknown address");

            #[cfg(feature = "metrics")]
            connection_metrics.connection_unknown_upstream.inc();

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

    let mut server_stream = match TcpStream::connect(upstream.addr())
        .instrument(trace_span!("connect_upstream"))
        .await
    {
        Ok(stream) => stream,
        Err(error) => {
            error!(
                %error,
                "could not connect to upstream"
            );

            #[cfg(feature = "metrics")]
            connection_metrics
                .connection_can_not_reach_upstream
                .get_or_create(&upstream.into())
                .inc();

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

    #[cfg(feature = "metrics")]
    connection_metrics
        .connection_established
        .get_or_create(&upstream.clone().into())
        .inc();

    // Forward the handshake to the upstream
    write_packet(
        &mut server_stream,
        handshake_packet.id,
        &handshake_packet.data,
    )
    .await?;

    trace!("passing upstream to proxy");

    Ok(ControlFlow::Continue((server_stream, upstream, handshake)))
}
