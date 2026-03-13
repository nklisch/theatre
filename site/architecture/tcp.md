# TCP Protocol Deep Dive

A detailed look at Theatre's TCP protocol: framing, connection lifecycle, error handling, and implementation.

## Why TCP?

Theatre uses TCP rather than Unix sockets or shared memory for several reasons:

1. **Cross-platform**: TCP works identically on Linux, macOS, and Windows
2. **Process boundary**: Server and game are separate processes; TCP is the natural boundary
3. **Framing**: TCP is a byte stream; length-prefix framing gives us message boundaries
4. **Debugging**: TCP traffic can be inspected with Wireshark or tcpdump during development

Compared to HTTP: TCP is lower latency (no HTTP overhead), simpler to implement in GDScript/GDExtension, and does not require an HTTP server in the game.

## Length-prefix framing

TCP is a stream protocol — there are no inherent message boundaries. A single `send()` call can be received as multiple `recv()` calls, or multiple sends can arrive as one receive.

Theatre solves this with a 4-byte big-endian length prefix before every message:

```
┌────────────────────────────┐
│  length (4 bytes, BE u32)  │
├────────────────────────────┤
│  JSON payload (N bytes)    │
└────────────────────────────┘
```

The decoder always:
1. Reads exactly 4 bytes → `length`
2. Reads exactly `length` bytes → payload
3. Parses payload as UTF-8 JSON

This guarantees exactly one JSON message per decode operation, regardless of TCP segmentation.

### Maximum message size

The maximum message size is 16 MB (enforced in the decoder):

```rust
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

if length > MAX_MESSAGE_SIZE {
    return Err(CodecError::MessageTooLarge { length });
}
```

This prevents a malformed length field (e.g., `0xFFFFFFFF`) from causing the decoder to allocate 4 GB of memory.

In practice, Theatre messages are much smaller — even a large snapshot response is rarely over 100 KB.

## Spectator connection lifecycle

### Initial connection

```
Server                          Addon (Godot)
  │                                 │
  │     TCP connect (port 9077)     │
  │ ──────────────────────────────► │
  │                                 │
  │  ◄── handshake message ─────── │
  │  {                              │
  │    "type": "handshake",         │
  │    "version": "0.1.0",         │
  │    "godot_version": "4.3",     │
  │    "project": "my-game"         │
  │  }                              │
  │                                 │
  │  ──── handshake_ack ──────────► │
  │  { "type": "handshake_ack",    │
  │    "version": "0.1.0" }        │
  │                                 │
  │  ◄── request/response ────────  │
  │       (normal operation)        │
```

The addon sends the handshake immediately after accepting the connection — before any request is made. This allows the server to validate version compatibility.

### Persistent connection

The server keeps the TCP connection open across multiple tool calls. Each tool call is a request-response pair on the existing connection:

```
[Tool call 1]  server → request → addon → response → server
[Tool call 2]  server → request → addon → response → server
[Tool call 3]  server → request → addon → response → server
```

The server uses a background tokio task to manage the connection. Tool handlers send requests via a channel and receive responses via `oneshot` channels, allowing the connection management to be decoupled from the MCP handler logic.

### Request-response matching

Since multiple tool calls could in theory be in-flight simultaneously, the server tags each request with a `request_id`:

```json
{"type": "snapshot", "request_id": "req_001", "detail": "summary"}
```

The addon echoes the `request_id` in its response:

```json
{"result": "ok", "request_id": "req_001", "frame": 412, "nodes": {...}}
```

The session state stores a map of `request_id → oneshot::Sender`. When a response arrives with a given `request_id`, the background task looks up the sender and delivers the response.

In practice, the Godot main thread processes one request at a time (no concurrency on the addon side), so there is rarely more than one in-flight request. But the matching ensures correctness if requests arrive in a burst.

### Reconnection

When the game exits, the TCP connection drops. The server detects this (read returns EOF or error) and marks the session as disconnected.

On the next tool call, the server attempts to reconnect. If the game is running again (new session), the server connects and exchanges handshakes. The frame count resets (new game session = frame 0). The server informs the agent that the session has reset.

### Timeout

Tool call requests have a 5-second timeout. If the addon does not respond within 5 seconds (e.g., the game is hung in an infinite loop), the server returns a timeout error to the agent:

```
"Operation timed out after 5000ms. The game may be paused or hung."
```

## Director connection lifecycle

Director uses a simpler connection model — one connection per operation (not persistent):

```
[Tool call]
  → director binary connects to port 6550 (or 6551)
  → sends operation JSON
  → reads response JSON
  → closes connection

[Next tool call]
  → new connection
  → ...
```

No handshake is needed for Director — the first message is the operation request. The connection is closed after the response is received.

This stateless model means the Director backend is completely stateless — any operation can be routed to any connection without context from previous operations.

## Codec implementation

The codec is in `crates/spectator-protocol/src/codec.rs`. There are four functions:

```rust
// Synchronous write (for GDExtension — no async runtime)
pub fn write_message(writer: &mut impl Write, payload: &[u8]) -> Result<(), CodecError> {
    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)?;
    Ok(())
}

// Synchronous read (for GDExtension)
pub fn read_message(reader: &mut impl Read) -> Result<Vec<u8>, CodecError> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_MESSAGE_SIZE {
        return Err(CodecError::MessageTooLarge { length: len });
    }
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

// Async write (for spectator-server — tokio runtime)
#[cfg(feature = "async")]
pub async fn write_message_async(stream: &mut TcpStream, payload: &[u8]) -> Result<(), CodecError> {
    let len = payload.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(payload).await?;
    Ok(())
}

// Async read (for spectator-server)
#[cfg(feature = "async")]
pub async fn read_message_async(stream: &mut TcpStream) -> Result<Vec<u8>, CodecError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_MESSAGE_SIZE {
        return Err(CodecError::MessageTooLarge { length: len });
    }
    let mut payload = vec![0u8; len as usize];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}
```

The sync and async variants are kept as close to identical as possible. Tests run both variants against the same fixtures.

## Error handling

### `CodecError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Message too large: {length} bytes (max 16MB)")]
    MessageTooLarge { length: u32 },

    #[error("Connection closed")]
    ConnectionClosed,
}
```

`Io(EOF)` is mapped to `ConnectionClosed` for cleaner error handling upstream.

### Server error layering

```
CodecError (protocol)
    ↓ wrapped by
TcpSessionError (session.rs)
    ↓ wrapped by
anyhow::Error (tool handlers)
    ↓ converted to
McpError::internal_error (tool response)
```

Connection errors become agent-visible error messages explaining the game is not running. Protocol errors become internal errors (should not happen in normal operation — they indicate a version mismatch or memory corruption).
