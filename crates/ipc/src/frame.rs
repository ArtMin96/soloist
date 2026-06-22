//! Length-prefixed JSON framing over any async byte stream.
//!
//! A frame is a 4-byte big-endian byte count followed by that many bytes of JSON. Both
//! ends share this one implementation, and [`MAX_FRAME`] caps a single message so a
//! malformed or hostile length prefix can never make the reader allocate without bound
//! (every buffer has a ceiling).

use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The largest single framed message, in bytes (8 MiB) — generous for a read-model
/// snapshot, but a hard ceiling on what one prefix can ask the reader to allocate.
pub const MAX_FRAME: u32 = 8 * 1024 * 1024;

/// Why a frame could not be written or read.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    /// The underlying stream errored.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// The message exceeds [`MAX_FRAME`] (when writing) or its prefix does (when reading).
    #[error("message exceeds the frame size limit")]
    TooLarge,
    /// The payload was not valid JSON for the expected type.
    #[error("malformed message: {0}")]
    Codec(#[from] serde_json::Error),
}

/// Writes one length-prefixed JSON frame and flushes it.
pub async fn write_frame<W, T>(writer: &mut W, message: &T) -> Result<(), FrameError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let body = serde_json::to_vec(message)?;
    let len = u32::try_from(body.len()).map_err(|_| FrameError::TooLarge)?;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads one length-prefixed JSON frame. Returns `Ok(None)` on a clean EOF before any
/// bytes (the peer closed the stream); `Err` on a partial, oversized, or malformed frame.
pub async fn read_frame<R, T>(reader: &mut R) -> Result<Option<T>, FrameError>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes).await {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }
    let len = u32::from_be_bytes(len_bytes);
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    let mut body = vec![0u8; len as usize];
    reader.read_exact(&mut body).await?;
    Ok(Some(serde_json::from_slice(&body)?))
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
