use std::convert::TryInto;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing_error::{InstrumentResult, TracedError};

use crate::proto::{response::StatusResponse, string, var_int, Handshake, NextState, Packet};

pub mod response;

/// Write a packet and output its data
#[tracing::instrument(skip(stream, data), fields(len=data.len()), err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn write_packet(
    stream: &mut TcpStream,
    id: i32,
    data: &[u8],
) -> Result<Packet, TracedError<io::Error>> {
    let ser_id = var_int::write(id);
    let length = (data.len() + ser_id.len()).try_into().unwrap();

    stream
        .write_all(&var_int::write(length))
        .await
        .in_current_span()?;
    stream.write_all(&ser_id).await.in_current_span()?;
    stream.write_all(data).await.in_current_span()?;

    Ok(Packet {
        id,
        length,
        data: Vec::from(data),
    })
}

/// Read a packet and output its data
#[tracing::instrument(skip(stream), err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn read_packet(stream: &mut TcpStream) -> Result<Packet, TracedError<io::Error>> {
    let length = var_int::read(stream).await.in_current_span()?.value;
    let id = var_int::read(stream).await.in_current_span()?;

    let mut data = vec![0u8; (length - id.length).try_into().unwrap()];
    stream.read_exact(&mut data).await.in_current_span()?;

    Ok(Packet {
        length,
        id: id.value,
        data,
    })
}

/// Read the handshake packet in and return the data from it
#[tracing::instrument(skip(stream), err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn read_handshake(
    stream: &mut TcpStream,
) -> Result<(Handshake, Packet), TracedError<io::Error>> {
    let packet = read_packet(stream).await?;
    assert_eq!(packet.id, 0x00);
    let mut data_buf = packet.data.as_slice();

    // Get the protocol version
    let protocol_version = var_int::read(&mut data_buf).await.in_current_span()?.value;
    let address = string::read(&mut data_buf).await.in_current_span()?;
    let port = data_buf.read_u16().await.in_current_span()?;
    let next_state = var_int::read(&mut data_buf).await.in_current_span()?.value;

    Ok((
        Handshake {
            protocol_version,
            address,
            port,
            next_state: NextState::from(next_state),
        },
        packet,
    ))
}

#[tracing::instrument(skip(stream), err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn read_ping_request(stream: &mut TcpStream) -> Result<i64, TracedError<io::Error>> {
    let packet = read_packet(stream).await?;
    assert_eq!(packet.id, 0x01);
    let mut data_buf = packet.data.as_slice();

    let payload = data_buf.read_i64().await.in_current_span()?;

    Ok(payload)
}

#[tracing::instrument(skip(stream), err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn write_status_response(
    stream: &mut TcpStream,
    response: &StatusResponse,
) -> Result<Packet, TracedError<io::Error>> {
    let response = string::write(&serde_json::to_string(response).unwrap());

    write_packet(stream, 0x00, &response).await
}

#[tracing::instrument(skip(stream), err)]
#[cfg_attr(feature = "autometrics", autometrics::autometrics)]
pub async fn write_pong_response(
    stream: &mut TcpStream,
    payload: i64,
) -> Result<Packet, TracedError<io::Error>> {
    write_packet(stream, 0x01, &payload.to_be_bytes()).await
}
