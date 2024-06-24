use std::{io::ErrorKind, marker::Unpin};

use tokio::io::{self, AsyncRead, AsyncReadExt};

/// Parse in a var int and return the value and its length
pub async fn read<T>(stream: &mut T) -> Result<VarInt, io::Error>
where
    T: AsyncRead + Unpin,
{
    let mut length: i32 = 0;
    let mut result: i32 = 0;
    let mut read: u8;

    while {
        read = stream.read_u8().await?;
        let value = read & 0b0111_1111;
        result |= (i32::from(value)) << (7 * length);

        length += 1;
        if length > 5 {
            return Err(io::Error::new(ErrorKind::InvalidInput, "VarInt is too big"));
        }

        (read & 0b1000_0000) != 0
    } {}

    Ok(VarInt {
        value: result,
        length,
    })
}

pub struct VarInt {
    pub value: i32,
    pub length: i32,
}

/// Convert an integer to a var_int
pub fn write(value: i32) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut mut_val = value;

    while {
        let mut temp = (mut_val & 0b0111_1111) as u8;
        // Note: >>> means that the sign bit is shifted with the rest of the number rather than being left alone
        mut_val >>= 7;
        if mut_val != 0 {
            temp |= 0b1000_0000;
        }
        buf.push(temp);
        mut_val != 0
    } {}

    buf
}
