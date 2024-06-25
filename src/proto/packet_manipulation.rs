use std::convert::TryInto;
use std::marker::Unpin;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing_error::{InstrumentResult, TracedError};

use crate::proto::{response::Response, string, var_int, Handshake, NextState, Packet};

/// Additions of manupulating of MC packets to any Write + Read + Sized
impl<R: AsyncWrite + AsyncRead + Unpin> PacketManipulation for R {}

pub(crate) trait PacketManipulation: AsyncWrite + AsyncRead + Unpin + Sized {
    /// Write a packet and output its data
    #[tracing::instrument(skip(self), fields(writer = std::any::type_name::<Self>()))]
    async fn write_packet(
        &mut self,
        id: i32,
        data: &[u8],
    ) -> Result<Packet, TracedError<io::Error>> {
        let ser_id = var_int::write(id);
        let length = (data.len() + ser_id.len()).try_into().unwrap();

        self.write_all(&var_int::write(length))
            .await
            .in_current_span()?;
        self.write_all(&ser_id).await.in_current_span()?;
        self.write_all(data).await.in_current_span()?;

        Ok(Packet {
            id,
            length,
            data: Vec::from(data),
        })
    }

    /// Read a packet and output its data
    #[tracing::instrument(skip(self), fields(writer = std::any::type_name::<Self>()))]
    async fn read_packet(&mut self) -> Result<Packet, TracedError<io::Error>> {
        let length = var_int::read(self).await.in_current_span()?.value;
        let id = var_int::read(self).await.in_current_span()?;

        let mut data = vec![0u8; (length - id.length).try_into().unwrap()];
        self.read_exact(&mut data).await.in_current_span()?;

        Ok(Packet {
            length,
            id: id.value,
            data,
        })
    }

    /// Read the handshake packet in and return the data from it
    #[tracing::instrument(skip(self), fields(writer = std::any::type_name::<Self>()))]
    async fn read_handshake(&mut self) -> Result<(Handshake, Packet), TracedError<io::Error>> {
        let packet = self.read_packet().await?;
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

    #[tracing::instrument(skip(self), fields(writer = std::any::type_name::<Self>()))]
    async fn read_ping_request(&mut self) -> Result<i64, TracedError<io::Error>> {
        let packet = self.read_packet().await?;
        assert_eq!(packet.id, 0x01);
        let mut data_buf = packet.data.as_slice();

        let payload = data_buf.read_i64().await.in_current_span()?;

        Ok(payload)
    }

    #[tracing::instrument(skip(self), fields(writer = std::any::type_name::<Self>()))]
    async fn write_status_response(
        &mut self,
        response: &Response,
    ) -> Result<Packet, TracedError<io::Error>> {
        let response = string::write(&serde_json::to_string(response).unwrap());

        self.write_packet(0x00, &response).await
    }

    #[tracing::instrument(skip(self), fields(writer = std::any::type_name::<Self>()))]
    async fn write_pong_response(
        &mut self,
        payload: i64,
    ) -> Result<Packet, TracedError<io::Error>> {
        self.write_packet(0x01, &payload.to_be_bytes()).await
    }
}
