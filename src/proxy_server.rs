use tokio::{io, net::TcpStream};
use tracing::{trace, warn};

pub struct ProxyServer {
    server_stream: TcpStream,
    client_stream: TcpStream,
}

impl ProxyServer {
    pub fn new(server_stream: TcpStream, client_stream: TcpStream) -> Self {
        ProxyServer {
            server_stream,
            client_stream,
        }
    }

    #[tracing::instrument(name="copy_bidirectional", skip(self))]
    pub async fn start(mut self) {
        match io::copy_bidirectional(&mut self.client_stream, &mut self.server_stream).await {
            Ok((a_to_b, b_to_a)) => trace!(a_to_b, b_to_a, "stream closed successfully"),
            Err(error) => warn!(%error, "stream closed with error"),
        }
    }
}
