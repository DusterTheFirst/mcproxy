use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread::{self, Builder};

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

fn main() -> io::Result<()> {
    println!("Starting proxy");

    let mut address_map = HashMap::<String, u16>::new();
    address_map.insert("server.test.mc".to_owned(), 25567);
    address_map.insert("otherserver.test.mc".to_owned(), 25566);

    // TODO: String?
    let listener = TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        25565,
    ))
    .expect("Unable to bind to socket");

    println!("Listening for connections on port 25565");

    for stream in listener.incoming() {
        match stream {
            Ok(mut client_stream) => {
                let connection_id = client_stream.peer_addr()?.port();

                let address_map = address_map.clone();

                Builder::new().name(format!("#{}", connection_id)).spawn(
                    move || -> io::Result<()> {
                        let handle = thread::current();
                        let handle_name = handle.name().unwrap_or("<UNNAMED>");

                        println!("[{}] {}", handle_name, Green.paint("New connection"));

                        // First, the client sends a Handshake packet with its state set to 1.
                        let handshake: Handshake = client_stream.read_handshake()?;
                        // println!("{}", RGB(128, 128, 128).paint("HANDSHAKE"));
                        // println!("{}\n\n", handshake);
                        println!(
                            "[{}] Connection using Address: {} and Port: {}",
                            handle_name, handshake.address, handshake.port
                        );

                        // Handle invalid ports
                        let port = address_map.get(&handshake.address).unwrap();

                        println!(
                            "[{}] Attempting to connect to the server running on port {}",
                            handle_name, port
                        );
                        let mut server_stream = TcpStream::connect(SocketAddr::new(
                            IpAddr::V4(Ipv4Addr::new(73, 38, 152, 65)),
                            *port,
                        ))?;
                        println!("[{}] Connected to proxied server", handle_name);

                        // TODO: Utilize TcpStreams' peek to never have to hold packets
                        server_stream.write_packet(handshake.packet.id, &handshake.packet.data)?;

                        let mut client_read_stream = BufReader::new(client_stream.try_clone()?);
                        let mut client_write_stream = BufWriter::new(client_stream.try_clone()?);

                        let mut server_read_stream = BufReader::new(server_stream.try_clone()?);
                        let mut server_write_stream = BufWriter::new(server_stream.try_clone()?);

                        let server_thread = Builder::new()
                            .name(format!("#{} => Server:{}", connection_id, port))
                            .spawn(move || -> io::Result<()> {
                                let handle = thread::current();
                                let handle_name = handle.name().unwrap_or("<UNNAMED>");

                                while client_read_stream.is_open() {
                                    let mut buf = [0];

                                    client_read_stream.read_exact(&mut buf)?;
                                    server_write_stream.write_all(&buf)?;
                                    server_write_stream.flush()?;
                                }

                                println!("[{}] Stream Closed", handle_name);

                                Ok(())
                            })?;

                        let client_thread = Builder::new()
                            .name(format!("#{} => Client", connection_id))
                            .spawn(move || -> io::Result<()> {
                                let handle = thread::current();
                                let handle_name = handle.name().unwrap_or("<UNNAMED>");

                                while server_read_stream.is_open() {
                                    let mut buf = [0];

                                    server_read_stream.read_exact(&mut buf)?;
                                    client_write_stream.write_all(&buf)?;
                                    client_write_stream.flush()?;
                                }

                                println!("[{}] Stream Closed", handle_name);

                                Ok(())
                            })?;

                        // TODO: Manage threads, dont orphan
                        server_thread.join().unwrap()?;
                        client_thread.join().unwrap()?;

                        Ok(())
                    },
                )?;
            }
            Err(e) => eprintln!("{:?}", e),
        }
    }

    Ok(())
}
