use async_std::io;
use async_std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;
use std::collections::HashMap;

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
    println!("Starting proxy");

    // TODO: config file + cmd line opts
    let config = Config::load("./example/config.toml").await?;

    let server_responses = config.placeholder_server.responses.load().await?;

    let listener = TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        config.proxy.port,
    ))
    .await
    .expect("Unable to bind to socket");

    // Listen for incoming connections
    let mut incoming = listener.incoming();
    println!("Listening for connections on port {}", config.proxy.port);

    // Accept connections as they come in
    while let Some(stream) = incoming.next().await {
        match stream {
            Ok(client_stream) => {
                // Get the map of addresseses
                let address_map = config.servers.clone();

                // Get the responses that are possible
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
                            eprintln!("Error in handling connection: {}", e);
                        }
                    };
                });
            }
            Err(e) => eprintln!("Error connecting to client: {}", e),
        }
    }

    Ok(())
}

async fn handle_connection(
    connection_id: u16,
    address_map: HashMap<String, SocketAddr>,
    server_responses: PlaceholderServerResponsesParsed,
    mut client_stream: TcpStream,
) -> io::Result<()> {
    // TODO: Handle legacy ping
    println!("[{}] {}", connection_id, Green.paint("New connection"));

    // First, the client sends a Handshake packet with its state set to 1.
    let handshake: Handshake = client_stream.read_handshake().await?;
    // println!("{}", RGB(128, 128, 128).paint("HANDSHAKE"));
    // println!("{}\n\n", handshake);
    println!(
        "[{}] Connection using address: {} and port: {} with protocol version: {}",
        connection_id, handshake.address, handshake.port, handshake.protocol_version
    );

    // Load mapping
    let mapping = address_map.get(&handshake.address);

    // Handle mapping
    let address = match mapping {
        Some(a) => a,
        None => {
            println!(
                "[{}] No mapping exists for {}",
                connection_id, handshake.address
            );

            if let Some(response) = server_responses.no_mapping {
                println!(
                    "[{} => Client] Responding with no_mapping motd",
                    connection_id
                );
                client_stream.write_response(&response).await?;
            }

            return Ok(());
        }
    };

    println!(
        "[{}] Attempting to connect to the server running on address {}",
        connection_id, address
    );
    let mut server_stream = match TcpStream::connect(address).await {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!(
                "[{}] Failed to connect to proxied server: {}",
                connection_id, e
            );

            if let Some(response) = server_responses.offline {
                println!("[{} => Client] Responding with offline motd", connection_id);
                client_stream.write_response(&response).await?;
            }

            return Ok(());
        }
    };
    println!("[{}] Connected to proxied server", connection_id);

    // TODO: Utilize TcpStreams' peek to never have to hold packets
    server_stream
        .write_packet(handshake.packet.id, &handshake.packet.data)
        .await?;

    // Spin up constant proxy until the connection is complete
    ProxyServer::new(server_stream, client_stream, connection_id)
        .start()
        .await;

    println!("[{}] Connection closed", connection_id);

    Ok(())
}
