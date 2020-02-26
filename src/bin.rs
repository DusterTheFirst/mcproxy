#[macro_use]
extern crate log;

use async_std::io;
use async_std::net::{SocketAddr, TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;
use std::collections::HashMap;
use std::sync::Arc;

// TODO: use the one from redox os or whatever
use ansi_term::Color::*;

pub mod config;
pub mod proto;
pub mod proxy_server;

use config::{Config, PlaceholderServerResponsesParsed};
use proto::{Handshake, PacketManipulation};
use proxy_server::ProxyServer;

#[async_std::main]
async fn main() -> io::Result<()> {
    if cfg!(debug_assertions) {
        dotenv::from_filename(".dev.env").ok();
    } else {
        dotenv::dotenv().ok();
    }
    pretty_env_logger::init();

    info!("Starting proxy");

    // TODO: config file + cmd line opts
    let config: Config = Config::load("./example/config.toml").await?;

    let address_map = Arc::new(config.servers);
    let server_responses: Arc<PlaceholderServerResponsesParsed> = Arc::new(config.placeholder_server.responses.load().await?);

    let listener = TcpListener::bind(SocketAddr::new(
        config.proxy.address,
        config.proxy.port,
    ))
    .await
    .expect("Unable to bind to socket");

    // Listen for incoming connections
    let mut incoming = listener.incoming();
    info!("Listening for connections on port {} and address {}", config.proxy.port, config.proxy.address);

    // Accept connections as they come in
    while let Some(stream) = incoming.next().await {
        match stream {
            Ok(client_stream) => {
                // Clone pointers to the address map and server responses
                let address_map = address_map.clone();
                let server_responses = server_responses.clone();

                // Fork off the connection handling
                task::spawn(async {
                    // Get the connection id
                    let connection_id = client_stream.peer_addr().unwrap().port();

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
                });
            }
            Err(e) => error!("Error connecting to client: {}", e),
        }
    }

    Ok(())
}

async fn handle_connection(
    connection_id: u16,
    address_map: Arc<HashMap<String, SocketAddr>>,
    server_responses: Arc<PlaceholderServerResponsesParsed>,
    mut client_stream: TcpStream,
) -> io::Result<()> {
    // TODO: Handle legacy ping
    trace!("[{}] {}", connection_id, Green.paint("New connection"));

    // First, the client sends a Handshake packet with its state set to 1.
    let handshake: Handshake = client_stream.read_handshake().await?;
    trace!(
        "[{}] Connection using address: {} and port: {} with protocol version: {}",
        connection_id, handshake.address, handshake.port, handshake.protocol_version
    );

    // Load mapping
    let mapping = address_map.get(&handshake.address);

    // Handle mapping
    let address = match mapping {
        Some(a) => a,
        None => {
            warn!(
                "[{}] No mapping exists for {}",
                connection_id, handshake.address
            );

            if let Some(ref response) = server_responses.no_mapping {
                trace!(
                    "[{} => Client] Responding with no_mapping motd",
                    connection_id
                );
                client_stream.write_response(&response).await?;
            }

            return Ok(());
        }
    };

    trace!(
        "[{}] Attempting to connect to the server running on address {}",
        connection_id, address
    );
    let mut server_stream = match TcpStream::connect(address).await {
        Ok(stream) => stream,
        Err(e) => {
            error!(
                "[{}] Failed to connect to proxied server: {}",
                connection_id, e
            );

            if let Some(ref response) = server_responses.offline {
                trace!("[{} => Client] Responding with offline motd", connection_id);
                client_stream.write_response(&response).await?;
            }

            return Ok(());
        }
    };
    trace!("[{}] Connected to proxied server", connection_id);

    // TODO: Utilize TcpStreams' peek to never have to hold packets
    server_stream
        .write_packet(handshake.packet.id, &handshake.packet.data)
        .await?;

    // Spin up constant proxy until the connection is complete
    ProxyServer::new(server_stream, client_stream, connection_id)
        .start()
        .await;

    trace!("[{}] Connection closed", connection_id);

    Ok(())
}
