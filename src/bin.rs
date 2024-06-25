use console_subscriber::ConsoleLayer;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    io::{self, AsyncReadExt},
    net::{TcpListener, TcpStream},
};
use tracing::{
    debug, error, field, info, level_filters::LevelFilter, trace, trace_span, warn, Span,
};
use tracing_error::{ErrorLayer, InstrumentResult, TracedError};
use tracing_subscriber::{
    filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

use crate::config::{Config, PlaceholderServerResponsesParsed};
use crate::proto::PacketManipulation;
use crate::proxy_server::ProxyServer;

pub mod config;
pub mod proto;
pub mod proxy_server;

#[tokio::main]
async fn main() -> io::Result<()> {
    tracing_subscriber::Registry::default()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
        .with(ConsoleLayer::builder().with_default_env().spawn())
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
                tokio::task::Builder::new()
                    .name(&connection_id.to_string())
                    .spawn(async move {
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
                    })
                    .unwrap();
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
    let (handshake, handshake_packet) = client_stream.read_handshake().await?;
    debug!(
        address = handshake.address,
        port = handshake.port,
        protocol_version = handshake.protocol_version,
        next_state = ?handshake.next_state,
        "handshake received"
    );
    Span::current().record("address", &handshake.address);

    // Handle mapping
    let address = match address_map.get(&handshake.address) {
        Some(a) => a,
        None => {
            warn!("unknown address");

            let packet = client_stream.read_packet().await?;
            println!("{:?}", packet);

            if let Some(ref response) = server_responses.no_mapping {
                let mut response = response.clone();
                if let Some(ref mut players) = response.players {
                    for player in players.sample.iter_mut() {
                        if uuid::Uuid::try_parse(&player.id).unwrap().is_nil() {
                            player.id = uuid::Uuid::new_v4().to_string();
                        }
                    }
                    players.online = players.sample.len() as u16;
                    players.max = players.max.max(players.online);
                }
                trace!("sending with no_mapping motd");
                client_stream.write_response(&response).await?;
            }

            let packet = client_stream.read_ping_request().await?;
            println!("{:?}", packet);

            return Ok(());
        }
    };

    let mut server_stream = match TcpStream::connect(address).await {
        Ok(stream) => stream,
        Err(error) => {
            error!(
                %error,
                "could not connect to upstream"
            );

            if let Some(ref response) = server_responses.offline {
                trace!("sending offline motd");
                client_stream.write_response(response).await?;
            }

            return Ok(());
        }
    };
    trace!("connected to upstream");

    // TODO: Utilize TcpStreams' peek to never have to hold packets
    server_stream
        .write_packet(handshake_packet.id, &handshake_packet.data)
        .await?;

    // Spin up constant proxy until the connection is complete
    ProxyServer::new(server_stream, client_stream).start().await;

    trace!("Connection closed");

    Ok(())
}
