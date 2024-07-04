use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing_error::{InstrumentError, TracedError};

use crate::proto::{io::write_packet, response::StatusResponse, string, TextComponent};

use super::{read_packet, read_ping_request, write_pong_response, write_status_response};

#[tracing::instrument(skip_all, err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn ping_response(
    client_stream: &mut TcpStream,
    response: Option<&StatusResponse>,
) -> Result<(), TracedError<io::Error>> {
    // The client follows up with a Status Request packet. This packet has no fields. The client is also able to skip this part entirely and send a Ping Request instead.
    read_packet(client_stream).await?;

    if let Some(response) = response {
        // The server should respond with a Status Response packet.
        write_status_response(client_stream, response).await?;
    }

    // If the process is continued, the client will now send a Ping Request packet containing some payload which is not important.
    let packet = read_ping_request(client_stream).await?;
    // The server will respond with the Pong Response packet and then close the connection.
    write_pong_response(client_stream, packet).await?;

    client_stream
        .shutdown()
        .await
        .map_err(InstrumentError::in_current_span)?;

    Ok(())
}

#[tracing::instrument(skip_all, err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn login_response(
    client_stream: &mut TcpStream,
    response: Option<&TextComponent>,
) -> Result<(), TracedError<io::Error>> {
    // TODO: put this in a struct
    let packet = read_packet(client_stream).await?;
    assert_eq!(packet.id, 0x00);

    let mut data_buf = packet.data.as_slice();
    // TODO: no need for these to be async
    let name = string::read(&mut data_buf).await?;
    let uuid = data_buf.read_u128().await?;

    println!("{name}: {uuid:x?}");

    if let Some(response) = response {
        // TODO: I can totally mechanize the construction of packets, maybe look into that?
        write_packet(
            client_stream,
            0x00,
            &string::write(&serde_json::to_string(&response).unwrap()),
        )
        .await?;
    }

    client_stream
        .shutdown()
        .await
        .map_err(InstrumentError::in_current_span)?;

    Ok(())
}
