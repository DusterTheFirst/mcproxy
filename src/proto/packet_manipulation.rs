use byteorder::{BigEndian, ReadBytesExt};
use std::convert::TryInto;
use std::io::{self, Read, Write};

use crate::proto::{string, var_int, Handshake, NextState, Packet};

/// Additions of manupulating of MC packets to any Write + Read + Sized
impl<R: Write + Read + Sized> PacketManipulation for R {}

pub trait PacketManipulation: Write + Read + Sized {
    /// Write a packet and output its data
    fn write_packet(&mut self, id: i32, data: &[u8]) -> Result<Packet, io::Error> {
        let ser_id = var_int::write(id);
        let length = (data.len() + ser_id.len()).try_into().unwrap();
        let ser_length = var_int::write(length);

        self.write_all(&ser_length)?;
        self.write_all(&ser_id)?;
        self.write_all(&data)?;

        Ok(Packet {
            id,
            length,
            data: Vec::from(data),
        })
    }

    /// Read a packet and output its data
    fn read_packet(&mut self) -> Result<Packet, io::Error> {
        let length = var_int::read(self)?.value;
        let id = var_int::read(self)?;
        let mut data = vec![0u8; (length - id.length).try_into().unwrap()];

        self.read_exact(&mut data)?;

        Ok(Packet {
            length,
            id: id.value,
            data,
        })
    }

    /// Read the handshake packet in and return the data from it
    fn read_handshake(&mut self) -> Result<Handshake, io::Error> {
        let packet = self.read_packet()?;
        let mut data_buf = packet.data.as_slice();

        // Get the protocol version
        let protocol_version = var_int::read(&mut data_buf)?.value;
        let address = string::read(&mut data_buf)?;
        let port = data_buf.read_u16::<BigEndian>()?;
        let next_state = var_int::read(&mut data_buf)?.value;

        Ok(Handshake {
            packet,
            protocol_version,
            address,
            port,
            next_state: NextState::from(next_state),
        })
    }
}
