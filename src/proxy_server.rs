use tokio::{io, net::TcpStream};
use tracing::{trace, warn};

pub struct ProxyServer {
    server_stream: TcpStream,
    client_stream: TcpStream,
    id: u16,
}

impl ProxyServer {
    pub fn new(server_stream: TcpStream, client_stream: TcpStream, id: u16) -> Self {
        ProxyServer {
            server_stream,
            client_stream,
            id,
        }
    }

    pub async fn start(mut self) {
        match io::copy_bidirectional(&mut self.client_stream, &mut self.server_stream).await {
            Ok((a_to_b, b_to_a)) => trace!(
                connection = self.id,
                a_to_b,
                b_to_a,
                "[Client <=> Server] Stream Closed successfully"
            ),
            Err(e) => warn!(
                connection = self.id,
                "[Client <=> Server] Stream Closed with error: {}", e
            ),
        }
    }
}
