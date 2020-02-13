use byteorder::{BigEndian, ReadBytesExt};
use std::convert::TryInto;
use async_std::io::{self, Read, Write};
use async_trait::async_trait;
use async_std::prelude::*;
use std::marker::Unpin;

use crate::proto::{string, var_int, Handshake, NextState, Packet};

/// Additions of manupulating of MC packets to any Write + Read + Sized
impl<R: Write + Read + Unpin> PacketManipulation for R {}

#[async_trait]
pub trait PacketManipulation: Write + Read + Unpin + Sized {
    /// Write a packet and output its data
    async fn write_packet(&mut self, id: i32, data: &[u8]) -> Result<Packet, io::Error> {
        let ser_id = var_int::write(id);
        let length = (data.len() + ser_id.len()).try_into().unwrap();
        let ser_length = var_int::write(length);

        self.write_all(&ser_length).await?;
        self.write_all(&ser_id).await?;
        self.write_all(&data).await?;

        Ok(Packet {
            id,
            length,
            data: Vec::from(data),
        })
    }

    /// Read a packet and output its data
    async fn read_packet(&mut self) -> Result<Packet, io::Error> {
        let length = var_int::read(self).await?.value;
        let id = var_int::read(self).await?;
        let mut data = vec![0u8; (length - id.length).try_into().unwrap()];

        self.read_exact(&mut data).await?;

        Ok(Packet {
            length,
            id: id.value,
            data,
        })
    }

    /// Read the handshake packet in and return the data from it
    async fn read_handshake(&mut self) -> Result<Handshake, io::Error> {
        let packet = self.read_packet().await?;
        let mut data_buf = packet.data.as_slice();

        // Get the protocol version
        let protocol_version = var_int::read(&mut data_buf).await?.value;
        let address = string::read(&mut data_buf).await?;
        let port = data_buf.read_u16::<BigEndian>()?;
        let next_state = var_int::read(&mut data_buf).await?.value;

        Ok(Handshake {
            packet,
            protocol_version,
            address,
            port,
            next_state: NextState::from(next_state),
        })
    }
}
