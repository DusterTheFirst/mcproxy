use tokio::io::{self, AsyncRead, AsyncReadExt};
use tracing_error::TracedError;

use crate::proto::var_int;
use std::convert::TryInto;
use std::marker::Unpin;

// Generate a UTF 8 string with a var_int size prefix
#[tracing::instrument]
pub fn write(string: &str) -> Vec<u8> {
    let mut string_vec = Vec::new();

    string_vec.extend(var_int::write(string.len().try_into().unwrap()));
    string_vec.extend_from_slice(string.as_bytes());

    string_vec
}

// Read a UTF 8 string with a var_int size prefix
#[tracing::instrument(skip_all)]
pub async fn read(
    stream: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<String, TracedError<io::Error>> {
    let len = var_int::read(stream).await?.value;
    let mut buf = vec![0u8; len.try_into().unwrap()];
    stream.read_exact(&mut buf).await?;

    Ok(String::from_utf8_lossy(&buf).to_string())
}
