use async_std::io;
use async_std::net::TcpStream;
use async_std::prelude::*;
use std::sync::Arc;

pub struct ProxyServer {
    server_stream: Arc<TcpStream>,
    client_stream: Arc<TcpStream>,
    id: u16,
}

impl ProxyServer {
    pub fn new(server_stream: TcpStream, client_stream: TcpStream, id: u16) -> Self {
        ProxyServer {
            server_stream: Arc::new(server_stream),
            client_stream: Arc::new(client_stream),
            id,
        }
    }

    pub async fn start(self) {
        let server_task = async {
            match io::copy(self.client_stream.as_ref(), self.server_stream.as_ref()).await {
                Ok(_) => trace!("[{}][Client => Server] Stream Closed successfully", self.id),
                Err(e) => warn!(
                    "[{}][Client => Server] Stream Closed with error: {}",
                    self.id, e
                ),
            }
        };
        let client_task = async {
            match io::copy(self.server_stream.as_ref(), self.client_stream.as_ref()).await {
                Ok(_) => trace!("[{}][Server => Client] Stream Closed successfully", self.id),
                Err(e) => warn!(
                    "[{}][Server => Client] Stream Closed with error: {}",
                    self.id, e
                ),
            }
        };

        client_task.join(server_task).await;
    }
}
