use std::{io::ErrorKind, marker::Unpin};

use tokio::io::{self, AsyncRead, AsyncReadExt};
use tracing_error::{InstrumentError, TracedError};

/// Parse in a var int and return the value and its length
#[tracing::instrument(skip_all)]
pub async fn read(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<VarInt, TracedError<io::Error>> {
    let mut length: i32 = 0;
    let mut result: i32 = 0;

    loop {
        let read = stream.read_u8().await?;
        let value = read & 0b0111_1111;
        result |= (i32::from(value)) << (7 * length);

        length += 1;
        if length > 5 {
            return Err(
                io::Error::new(ErrorKind::InvalidInput, "VarInt is too big").in_current_span()
            );
        }

        if (read & 0b1000_0000) == 0 {
            break;
        }
    }

    Ok(VarInt {
        value: result,
        length,
    })
}

#[derive(Debug)]
pub struct VarInt {
    pub value: i32,
    pub length: i32,
}

/// Convert an integer to a var_int
#[tracing::instrument]
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
