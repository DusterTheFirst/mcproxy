use mcproxy_model::Hostname;
use std::convert::TryInto;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::Span;
use tracing_error::{InstrumentResult, TracedError};

use crate::proto::{response::StatusResponse, string, var_int, Handshake, NextState, Packet};

pub mod request;
pub mod response;

/// Write a packet and output its data
#[tracing::instrument(skip(stream, data), fields(len=data.len()))]
pub async fn write_packet(
    stream: &mut (dyn AsyncWrite + Unpin + Send),
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

#[tracing::instrument(skip(stream, vectors), fields(len=tracing::field::Empty))]
pub async fn write_packet_vectored(
    stream: &mut (dyn AsyncWrite + Unpin + Send),
    id: i32,
    vectors: &[&[u8]],
) -> Result<Packet, TracedError<io::Error>> {
    let data_length: usize = vectors.iter().map(|v| v.len()).sum();
    Span::current().record("len", data_length);

    let ser_id = var_int::write(id);
    let length = (data_length + ser_id.len()).try_into().unwrap();

    stream.write_all(&var_int::write(length)).await?;
    stream.write_all(&ser_id).await?;
    for vector in vectors {
        stream.write_all(vector).await?;
    }

    Ok(Packet {
        id,
        length,
        data: Vec::new(), // FIXME: why are we re-allocating??
    })
}

/// Read a packet and output its data
#[tracing::instrument(skip(stream))]
pub async fn read_packet(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<Packet, TracedError<io::Error>> {
    let length = var_int::read(stream).await?.value;
    let id = var_int::read(stream).await?;

    let mut data = vec![0u8; (length - id.length).try_into().unwrap()];
    stream.read_exact(&mut data).await.in_current_span()?;

    Ok(Packet {
        length,
        id: id.value,
        data,
    })
}

/// Read the handshake packet in and return the data from it
#[tracing::instrument(skip(stream))]
pub async fn read_handshake(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<(Handshake, Packet), TracedError<io::Error>> {
    let packet = read_packet(stream).await?;

    assert_eq!(packet.id, 0x00); // FIXME: should I panic?

    let mut data_buf = packet.data.as_slice();

    // Get the protocol version
    let protocol_version = var_int::read(&mut data_buf).await?.value;
    let address = string::read(&mut data_buf).await?;
    let port = data_buf.read_u16().await.in_current_span()?;
    let next_state = var_int::read(&mut data_buf).await?.value;

    let mut parts = address.split_terminator('\0');
    let address = parts.next().expect("first part should always exist");
    let address_forge = parts.next(); // https://wiki.vg/Minecraft_Forge_Handshake#Changes_to_Handshake_packet
    assert_eq!(parts.next(), None);

    Ok((
        Handshake {
            protocol_version,
            address: Hostname::from(address),
            address_forge_version: address_forge.map(String::from),
            port,
            next_state: NextState::from(next_state),
        },
        packet,
    ))
}

#[tracing::instrument(skip(stream))]
pub async fn write_handshake(
    stream: &mut (dyn AsyncWrite + Unpin + Send),
    handshake: Handshake,
) -> Result<Packet, TracedError<io::Error>> {
    let address = if let Some(forge_version) = handshake.address_forge_version {
        &[handshake.address.as_ref(), "\0", &forge_version, "\0"].concat()
    } else {
        handshake.address.as_ref()
    };

    let protocol_version = var_int::write(handshake.protocol_version);
    let address = string::write(address);
    let port = u16::to_be_bytes(handshake.port);
    let next_state = var_int::write(handshake.next_state.into());

    write_packet_vectored(
        stream,
        0x00,
        &[&protocol_version, &address, &port[..], &next_state],
    )
    .await
}

#[tracing::instrument(skip(stream))]
pub async fn read_status_request(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<(), TracedError<io::Error>> {
    let packet = read_packet(stream).await?;

    assert_eq!(packet.id, 0x00, "unexpected packet id");
    assert_eq!(packet.length, 1, "unexpected packet length");

    Ok(())
}

#[tracing::instrument(skip(stream))]
pub async fn write_status_request(
    stream: &mut (dyn AsyncWrite + Unpin + Send),
) -> Result<Packet, TracedError<io::Error>> {
    write_packet(stream, 0x00, &[]).await
}

#[tracing::instrument(skip(stream))]
pub async fn read_status_response(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<StatusResponse, TracedError<io::Error>> {
    let packet = read_packet(stream).await?;

    assert_eq!(packet.id, 0x00);

    let mut data_buf = packet.data.as_slice();

    let response = string::read(&mut data_buf).await?;
    let response = serde_json::from_str(&response).unwrap();

    Ok(response)
}

#[tracing::instrument(skip(stream), err)]
pub async fn write_status_response(
    stream: &mut (dyn AsyncWrite + Unpin + Send),
    response: &StatusResponse,
) -> Result<Packet, TracedError<io::Error>> {
    let response = string::write(&serde_json::to_string(response).unwrap());

    write_packet(stream, 0x00, &response).await
}

#[tracing::instrument(skip(stream), err)]
pub async fn read_ping_pong(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<i64, TracedError<io::Error>> {
    let packet = read_packet(stream).await?;
    assert_eq!(packet.id, 0x01);
    let mut data_buf = packet.data.as_slice();

    let payload = data_buf.read_i64().await.in_current_span()?;

    Ok(payload)
}

#[tracing::instrument(skip(stream), err)]
pub async fn write_ping_pong(
    stream: &mut (dyn AsyncWrite + Unpin + Send),
    payload: i64,
) -> Result<Packet, TracedError<io::Error>> {
    write_packet(stream, 0x01, &payload.to_be_bytes()).await
}
