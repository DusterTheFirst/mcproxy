use async_std::io;
use async_std::net::TcpStream;
use async_std::prelude::*;

pub struct ProxyServer {
    server_stream: TcpStream,
    client_stream: TcpStream,
    id: u16,
}

impl ProxyServer {
    pub const fn new(server_stream: TcpStream, client_stream: TcpStream, id: u16) -> Self {
        ProxyServer {
            server_stream,
            client_stream,
            id,
        }
    }

    pub async fn start(self) {
        let server_task = async {
            match io::copy(self.client_stream.clone(), self.server_stream.clone()).await {
                Ok(_) => println!("[{}][Client => Server] Stream Closed successfully", self.id),
                Err(e) => eprint!(
                    "[{}][Client => Server] Stream Closed with error: {}",
                    self.id, e
                ),
            }
        };
        let client_task = async {
            match io::copy(self.server_stream.clone(), self.client_stream.clone()).await {
                Ok(_) => println!("[{}][Server => Client] Stream Closed successfully", self.id),
                Err(e) => eprint!(
                    "[{}][Server => Client] Stream Closed with error: {}",
                    self.id, e
                ),
            }
        };

        client_task.join(server_task).await;
    }
}
