use async_std::io;
use async_std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;
use std::collections::HashMap;

// TODO: use the one from redox os or whatever
use ansi_term::Color::*;

pub mod proto;
pub mod read;

pub use read::IsOpen;

use proto::{Handshake, PacketManipulation};

pub use proto::{
    response::{Player, Players},
    Chat,
};

// pub struct ProxyServer {
//     server_stream: TcpStream,
//     client_stream: TcpStream
// }

// impl ProxyServer {
//     pub fn new(server_stream: TcpStream, client_stream: TcpStream) -> ProxyServer {
//         ProxyServer {
//             server_stream,
//             client_stream
//         }
//     }
// }

#[async_std::main]
async fn main() -> io::Result<()> {
    println!("Starting proxy");

    // TODO: config file + cmd line opts
    let mut address_map = HashMap::<String, SocketAddr>::new();
    address_map.insert(
        "0.mcproxy.dusterthefirst.com".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25570),
    );
    address_map.insert(
        "1.mcproxy.dusterthefirst.com".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25571),
    );
    address_map.insert(
        "2.mcproxy.dusterthefirst.com".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25572),
    );
    address_map.insert(
        "3.mcproxy.dusterthefirst.com".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25573),
    );
    address_map.insert(
        "4.mcproxy.dusterthefirst.com".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25574),
    );
    address_map.insert(
        "5.mcproxy.dusterthefirst.com".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25575),
    );
    address_map.insert(
        "localhost".to_owned(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25580),
    );

    // TODO: String?
    let listener = TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        25565,
    ))
    .await
    .expect("Unable to bind to socket");

    let mut incoming = listener.incoming();

    println!("Listening for connections on port 25565");

    while let Some(stream) = incoming.next().await {
        match stream {
            Ok(client_stream) => {
                let address_map = address_map.clone();

                task::spawn(async {
                    let connection_id = client_stream.peer_addr().unwrap().port();

                    let res = handle_connection(connection_id, address_map, client_stream).await;
                    match res {
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

    // Handle invalid ports
    let mapping = address_map.get(&handshake.address);

    let address = match mapping {
        Some(a) => a,
        None => {
            println!(
                "[{}] No mapping exists for {}",
                connection_id, handshake.address
            );

            // TODO: dummy motd

            return Ok(());
        }
    };

    println!(
        "[{}] Attempting to connect to the server running on address {}",
        connection_id, address
    );
    let mut server_stream = TcpStream::connect(address).await?;
    println!("[{}] Connected to proxied server", connection_id);

    // TODO: Utilize TcpStreams' peek to never have to hold packets
    server_stream
        .write_packet(handshake.packet.id, &handshake.packet.data)
        .await?;

    let server_task = async {
        match io::copy(client_stream.clone(), server_stream.clone()).await {
            Ok(_) => println!("[{} => Server] Stream Closed successfully", connection_id),
            Err(e) => eprint!(
                "[{} => Server] Stream Closed with error: {}",
                connection_id, e
            ),
        }
    };

    let client_task = async {
        match io::copy(server_stream.clone(), client_stream.clone()).await {
            Ok(_) => println!("[{} => Client] Stream Closed successfully", connection_id),
            Err(e) => eprint!(
                "[{} => Client] Stream Closed with error: {}",
                connection_id, e
            ),
        }
    };

    client_task.join(server_task).await;

    Ok(())
}
