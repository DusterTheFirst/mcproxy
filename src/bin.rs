use proto::{
    io::{
        read_handshake, read_packet, read_ping_request, write_packet, write_pong_response,
        write_status_response,
    },
    response::Response,
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task,
};
use tracing::{debug, error, field, info, trace, warn, Span};
use tracing_error::{ErrorLayer, InstrumentResult, TracedError};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::config::{Config, PlaceholderServerResponsesParsed};
use crate::proxy_server::ProxyServer;

pub mod config;
pub mod proto;
pub mod proxy_server;

#[tokio::main]
async fn main() -> io::Result<()> {
    #[cfg(not(feature = "tokio-console"))]
    let console_layer = tracing_subscriber::layer::Identity::new();

    #[cfg(feature = "tokio-console")]
    let console_layer = console_subscriber::ConsoleLayer::builder()
        .with_default_env()
        .spawn();

    tracing_subscriber::Registry::default()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
        .with(console_layer)
        .init();

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
            Ok((client_stream, address)) => {
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
                        client_stream,
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Error in handling connection: {}", e);
                        }
                    };
                };

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

#[tracing::instrument(name="connection", skip_all, fields(connection=connection_id, address=field::Empty))]
async fn handle_connection(
    connection_id: u16,
    address_map: Arc<HashMap<String, SocketAddr>>,
    server_responses: Arc<PlaceholderServerResponsesParsed>,
    mut client_stream: TcpStream,
) -> Result<(), TracedError<io::Error>> {
    // TODO: Handle legacy ping
    trace!("new connection");

    // First, the client sends a Handshake packet with its state set to 1.
    let (handshake, handshake_packet) = read_handshake(&mut client_stream).await?;
    debug!(
        address = handshake.address,
        port = handshake.port,
        protocol_version = handshake.protocol_version,
        next_state = ?handshake.next_state,
        "handshake received"
    );
    Span::current().record("address", &handshake.address);

    // match handshake.next_state {
    //     proto::NextState::Ping => {
    //         // TODO: ping fn
    //     }
    //     proto::NextState::Connect => todo!(),
    //     proto::NextState::Unknown(state) => warn!(state, "unknown next_state"),
    // }

    // Handle mapping
    let address = match address_map.get(&handshake.address) {
        Some(a) => a,
        None => {
            warn!("unknown address");

            return ping_response(client_stream, server_responses.no_mapping.as_ref()).await;
        }
    };

    let mut server_stream = match TcpStream::connect(address).await {
        Ok(stream) => stream,
        Err(error) => {
            error!(
                %error,
                "could not connect to upstream"
            );

            return ping_response(client_stream, server_responses.offline.as_ref()).await;
        }
    };
    trace!("connected to upstream");

    // TODO: Utilize TcpStreams' peek to never have to hold packets
    write_packet(
        &mut server_stream,
        handshake_packet.id,
        &handshake_packet.data,
    )
    .await?;

    // Spin up constant proxy until the connection is complete
    ProxyServer::new(server_stream, client_stream).start().await;

    trace!("Connection closed");

    Ok(())
}

async fn ping_response(
    mut client_stream: TcpStream,
    response: Option<&Response>,
) -> Result<(), TracedError<io::Error>> {
    // The client follows up with a Status Request packet. This packet has no fields. The client is also able to skip this part entirely and send a Ping Request instead.
    read_packet(&mut client_stream).await?;

    if let Some(response) = response {
        // The server should respond with a Status Response packet.
        write_status_response(&mut client_stream, response).await?;
    }

    // If the process is continued, the client will now send a Ping Request packet containing some payload which is not important.
    let packet = read_ping_request(&mut client_stream).await?;
    // The server will respond with the Pong Response packet and then close the connection.
    write_pong_response(&mut client_stream, packet).await?;

    client_stream.shutdown().await.in_current_span()?;

    Ok(())
}
