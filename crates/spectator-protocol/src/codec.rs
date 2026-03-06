use serde::{Serialize, de::DeserializeOwned};
use std::io;

/// Maximum message payload size: 16 MiB.
pub const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Encode a message as length-prefixed JSON.
///
/// Format: [4 bytes: payload length, big-endian u32][JSON payload, UTF-8]
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, CodecError> {
    let json = serde_json::to_vec(msg).map_err(CodecError::Serialize)?;
    let len = json.len() as u32;
    if len > MAX_MESSAGE_SIZE {
        return Err(CodecError::MessageTooLarge(len));
    }
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&json);
    Ok(buf)
}

/// Read exactly `n` bytes from a synchronous reader.
pub fn read_exact(reader: &mut impl io::Read, n: usize) -> Result<Vec<u8>, CodecError> {
    let mut buf = vec![0u8; n];
    reader.read_exact(&mut buf).map_err(CodecError::Io)?;
    Ok(buf)
}

/// Read one length-prefixed JSON message from a synchronous reader.
pub fn read_message<T: DeserializeOwned>(reader: &mut impl io::Read) -> Result<T, CodecError> {
    let len_bytes = read_exact(reader, 4)?;
    let len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);
    if len > MAX_MESSAGE_SIZE {
        return Err(CodecError::MessageTooLarge(len));
    }
    let payload = read_exact(reader, len as usize)?;
    serde_json::from_slice(&payload).map_err(CodecError::Deserialize)
}

/// Write one length-prefixed JSON message to a synchronous writer.
pub fn write_message<T: Serialize>(
    writer: &mut impl io::Write,
    msg: &T,
) -> Result<(), CodecError> {
    let bytes = encode(msg)?;
    writer.write_all(&bytes).map_err(CodecError::Io)?;
    writer.flush().map_err(CodecError::Io)?;
    Ok(())
}

#[derive(Debug)]
pub enum CodecError {
    Io(io::Error),
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    MessageTooLarge(u32),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Serialize(e) => write!(f, "serialization error: {e}"),
            Self::Deserialize(e) => write!(f, "deserialization error: {e}"),
            Self::MessageTooLarge(n) => {
                write!(f, "message too large: {n} bytes (max {MAX_MESSAGE_SIZE})")
            }
        }
    }
}

impl std::error::Error for CodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Serialize(e) | Self::Deserialize(e) => Some(e),
            Self::MessageTooLarge(_) => None,
        }
    }
}

/// Async codec functions for use with tokio::net::TcpStream.
/// These use tokio::io::{AsyncReadExt, AsyncWriteExt}.
#[cfg(feature = "async")]
pub mod async_io {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Read one length-prefixed JSON message from an async reader.
    pub async fn read_message<T: DeserializeOwned>(
        reader: &mut (impl AsyncReadExt + Unpin),
    ) -> Result<T, CodecError> {
        let mut len_bytes = [0u8; 4];
        reader
            .read_exact(&mut len_bytes)
            .await
            .map_err(CodecError::Io)?;
        let len = u32::from_be_bytes(len_bytes);
        if len > MAX_MESSAGE_SIZE {
            return Err(CodecError::MessageTooLarge(len));
        }
        let mut payload = vec![0u8; len as usize];
        reader
            .read_exact(&mut payload)
            .await
            .map_err(CodecError::Io)?;
        serde_json::from_slice(&payload).map_err(CodecError::Deserialize)
    }

    /// Write one length-prefixed JSON message to an async writer.
    pub async fn write_message<T: Serialize>(
        writer: &mut (impl AsyncWriteExt + Unpin),
        msg: &T,
    ) -> Result<(), CodecError> {
        let bytes = encode(msg)?;
        writer.write_all(&bytes).await.map_err(CodecError::Io)?;
        writer.flush().await.map_err(CodecError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn encode_produces_length_prefixed_json() {
        let msg = serde_json::json!({"hello": "world"});
        let bytes = encode(&msg).unwrap();
        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let json: serde_json::Value = serde_json::from_slice(&bytes[4..]).unwrap();
        assert_eq!(len as usize, bytes.len() - 4);
        assert_eq!(json["hello"], "world");
    }

    #[test]
    fn write_read_round_trip() {
        let original = serde_json::json!({"test": 42, "nested": {"a": true}});
        let mut buf = Vec::new();
        write_message(&mut buf, &original).unwrap();
        let mut cursor = Cursor::new(buf);
        let decoded: serde_json::Value = read_message(&mut cursor).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn rejects_oversized_message() {
        // Create a length prefix claiming 17 MiB
        let mut buf = Vec::new();
        let bad_len: u32 = 17 * 1024 * 1024;
        buf.extend_from_slice(&bad_len.to_be_bytes());
        buf.extend_from_slice(b"{}"); // tiny payload, but prefix says huge
        let mut cursor = Cursor::new(buf);
        let result = read_message::<serde_json::Value>(&mut cursor);
        assert!(matches!(result, Err(CodecError::MessageTooLarge(_))));
    }

    #[test]
    fn multiple_messages_in_stream() {
        let msg1 = serde_json::json!({"seq": 1});
        let msg2 = serde_json::json!({"seq": 2});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg1).unwrap();
        write_message(&mut buf, &msg2).unwrap();
        let mut cursor = Cursor::new(buf);
        let d1: serde_json::Value = read_message(&mut cursor).unwrap();
        let d2: serde_json::Value = read_message(&mut cursor).unwrap();
        assert_eq!(d1["seq"], 1);
        assert_eq!(d2["seq"], 2);
    }
}
