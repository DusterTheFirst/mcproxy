use std::{
    io,
    time::{Duration, Instant, SystemTime},
};

use mcproxy_model::{Hostname, Upstream};
use tokio::{
    io::{AsyncWriteExt, BufReader, BufWriter},
    net::TcpStream,
};
use tracing_error::{InstrumentError, TracedError};

use crate::proto::packet::{response::StatusResponse, Handshake, NextState};

use super::{
    read_ping_pong, read_status_response, write_handshake, write_ping_pong, write_status_request,
};

#[tracing::instrument(skip(client_stream))]
// TODO: provide context in error where the problem occurred....
pub async fn server_list_ping(
    mut client_stream: TcpStream,
    upstream: Upstream,
) -> Result<(Duration, StatusResponse), TracedError<io::Error>> {
    let (read, write) = client_stream.split();
    let (mut read, mut write) = (BufReader::new(read), BufWriter::new(write));

    write_handshake(
        &mut write,
        Handshake {
            protocol_version: 0,
            address: Hostname::from(upstream.host),
            port: upstream.port,
            next_state: NextState::Ping,

            address_forge_version: None,
        },
    )
    .await?;

    // The client follows up with a Status Request packet. This packet has no fields. The client is also able to skip this part entirely and send a Ping Request instead.
    write_status_request(&mut write).await?;

    // If the process is continued, the client will now send a Ping Request packet containing some payload which is not important.
    let sent_payload = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(42))
        .as_secs() as i64;
    write_ping_pong(&mut write, sent_payload).await?;
    let ping_sent = Instant::now();

    // Send written packets
    write.flush().await?;

    // The server should respond with a Status Response packet.
    let response = read_status_response(&mut read).await?;

    // The server will respond with the Pong Response packet and then close the connection.
    let received_payload = read_ping_pong(&mut read).await?;
    let ping = ping_sent.elapsed();

    if sent_payload != received_payload {
        return Err(io::Error::other("pong payload incorrect").in_current_span());
        // FIXME: show payload
    }

    client_stream
        .shutdown()
        .await
        .map_err(InstrumentError::in_current_span)?;

    Ok((ping, response))
}
