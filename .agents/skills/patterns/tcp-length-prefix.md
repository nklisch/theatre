# Pattern: Length-Prefixed TCP Codec

All TCP messages are framed as `[4-byte BE u32 length][JSON payload]` with a 16 MiB cap. The codec module provides both sync and async variants behind a feature flag.

## Rationale
Enables reliable message framing over a raw TCP stream. Sync variant used in stage-godot (main-thread Godot); async variant used in stage-server (tokio). Both share the same `encode()` and `CodecError` type.

## Examples

### Example 1: Codec implementation — encode + sync read/write
**File**: `crates/stage-protocol/src/codec.rs:10-46`
```rust
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

pub fn read_message<T: DeserializeOwned>(reader: &mut impl io::Read) -> Result<T, CodecError> {
    let len_bytes = read_exact(reader, 4)?;
    let len = u32::from_be_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);
    if len > MAX_MESSAGE_SIZE { return Err(CodecError::MessageTooLarge(len)); }
    let payload = read_exact(reader, len as usize)?;
    serde_json::from_slice(&payload).map_err(CodecError::Deserialize)
}
```

### Example 2: Async variant (feature-gated, used in stage-server)
**File**: `crates/stage-protocol/src/codec.rs:81-116`
```rust
#[cfg(feature = "async")]
pub mod async_io {
    pub async fn read_message<T: DeserializeOwned>(
        reader: &mut (impl AsyncReadExt + Unpin),
    ) -> Result<T, CodecError> { ... }

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
```

### Example 3: Codec used in server TCP connection handshake
**File**: `crates/stage-server/src/tcp.rs:127-130`
```rust
let msg: Message = async_io::read_message(&mut reader)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to read handshake: {}", e))?;
```

### Example 4: Codec used in godot TCP server (sync, main thread)
**File**: `crates/stage-godot/src/tcp_server.rs` (uses `codec::read_message` / `codec::write_message`)
```rust
use stage_protocol::{codec, handshake::Handshake, messages::Message};
// ...
codec::write_message(&mut stream, &Message::Handshake(handshake))?;
let ack: Message = codec::read_message(&mut stream)?;
```

## When to Use
- Any new TCP message type: wrap with `write_message` / `read_message`
- Adding async TCP I/O in stage-server: use `codec::async_io`
- Adding sync TCP I/O in stage-godot: use `codec` directly (sync)

## When NOT to Use
- Inter-thread communication within a single process — use channels instead
- Testing without a real stream — use `std::io::Cursor` as the reader/writer

## Common Violations
- Writing raw JSON without length prefix — the other end won't know where the message ends
- Allocating unbounded payload before checking `MAX_MESSAGE_SIZE` — always check the length prefix first
