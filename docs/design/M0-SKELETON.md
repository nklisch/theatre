# Design: Milestone 0 — Skeleton

## Overview

M0 establishes the repo structure, builds both Rust artifacts, makes the GDExtension loadable in Godot, and proves TCP communication works with a handshake exchange. Nothing useful yet — just proof the plumbing connects.

**Exit Criteria:** Run Godot with the addon enabled → hit Play → `spectator-server` connects → handshake logged on both sides → stop game → server reconnects when game restarts.

---

## Implementation Units

### Unit 1: Cargo Workspace & Crate Scaffolding

**Files:**
- `Cargo.toml` (workspace root)
- `crates/spectator-server/Cargo.toml`
- `crates/spectator-server/src/main.rs`
- `crates/spectator-godot/Cargo.toml`
- `crates/spectator-godot/src/lib.rs`
- `crates/spectator-protocol/Cargo.toml`
- `crates/spectator-protocol/src/lib.rs`
- `crates/spectator-core/Cargo.toml`
- `crates/spectator-core/src/lib.rs`

#### `Cargo.toml` (workspace root)

```toml
[workspace]
resolver = "2"
members = [
    "crates/spectator-server",
    "crates/spectator-godot",
    "crates/spectator-protocol",
    "crates/spectator-core",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/spectator-godot/spectator"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
spectator-protocol = { path = "crates/spectator-protocol" }
spectator-core = { path = "crates/spectator-core" }
```

#### `crates/spectator-server/Cargo.toml`

```toml
[package]
name = "spectator-server"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "spectator-server"
path = "src/main.rs"

[dependencies]
spectator-protocol.workspace = true
spectator-core.workspace = true
rmcp = { version = "0.16", features = ["server", "transport-io"] }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
schemars = "1"
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow = "1"
uuid = { version = "1", features = ["v4"] }
```

#### `crates/spectator-godot/Cargo.toml`

```toml
[package]
name = "spectator-godot"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
spectator-protocol.workspace = true
godot = { version = "0.4", features = ["api-4-2"] }
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
```

#### `crates/spectator-protocol/Cargo.toml`

```toml
[package]
name = "spectator-protocol"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
```

#### `crates/spectator-core/Cargo.toml`

```toml
[package]
name = "spectator-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
```

#### `crates/spectator-core/src/lib.rs`

```rust
//! Shared spatial logic for Spectator.
//!
//! Pure computation: bearing math, spatial indexing, delta engine, token budget.
//! No Godot API, no MCP API — testable standalone.
```

#### `crates/spectator-server/src/main.rs` (stub)

```rust
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("spectator=info".parse()?),
        )
        .init();

    tracing::info!("spectator-server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("waiting for connection...");

    Ok(())
}
```

#### `crates/spectator-godot/src/lib.rs` (stub)

```rust
use godot::prelude::*;

struct SpectatorExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SpectatorExtension {}
```

#### `crates/spectator-protocol/src/lib.rs` (stub)

```rust
//! TCP wire protocol types shared between spectator-server and spectator-godot.

pub mod codec;
pub mod handshake;
pub mod messages;
```

**Acceptance Criteria:**
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` succeeds (no tests yet, but no compilation errors)
- [ ] `cargo clippy --workspace` reports no warnings
- [ ] `cargo fmt --check` passes
- [ ] `spectator-server` binary runs, prints version and "waiting for connection"
- [ ] `spectator-godot` produces `libspectator_godot.so` (or platform equivalent)

---

### Unit 2: Protocol — Handshake Message Types

**File:** `crates/spectator-protocol/src/handshake.rs`

```rust
use serde::{Deserialize, Serialize};

/// Sent by the addon immediately after TCP connection is established.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Handshake {
    /// Always "handshake"
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Spectator addon version (e.g., "0.1.0")
    pub spectator_version: String,

    /// Wire protocol version. Must match between server and addon.
    pub protocol_version: u32,

    /// Godot engine version string (e.g., "4.3")
    pub godot_version: String,

    /// 2, 3, or 0 for mixed. Determined by scene root type.
    pub scene_dimensions: u32,

    /// Physics ticks per second (typically 60)
    pub physics_ticks_per_sec: u32,

    /// Godot project name from ProjectSettings
    pub project_name: String,
}

/// Sent by the server in response to a valid Handshake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeAck {
    /// Always "handshake_ack"
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Server's spectator version
    pub spectator_version: String,

    /// Agreed protocol version
    pub protocol_version: u32,

    /// Unique identifier for this session
    pub session_id: String,
}

/// Sent by the server when protocol versions are incompatible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeError {
    /// Always "handshake_error"
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Human-readable error description
    pub message: String,

    /// Server's spectator version
    pub server_version: String,

    /// Protocol versions the server supports
    pub supported_protocols: Vec<u32>,
}

/// Current protocol version. Incremented on breaking wire format changes.
pub const PROTOCOL_VERSION: u32 = 1;

impl Handshake {
    pub fn new(
        godot_version: String,
        scene_dimensions: u32,
        physics_ticks_per_sec: u32,
        project_name: String,
    ) -> Self {
        Self {
            msg_type: "handshake".to_string(),
            spectator_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
            godot_version,
            scene_dimensions,
            physics_ticks_per_sec,
            project_name,
        }
    }
}

impl HandshakeAck {
    pub fn new(session_id: String) -> Self {
        Self {
            msg_type: "handshake_ack".to_string(),
            spectator_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
            session_id,
        }
    }
}

impl HandshakeError {
    pub fn version_mismatch(addon_version: u32) -> Self {
        Self {
            msg_type: "handshake_error".to_string(),
            message: format!(
                "Protocol version mismatch: server supports v{}, addon sent v{}",
                PROTOCOL_VERSION, addon_version
            ),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            supported_protocols: vec![PROTOCOL_VERSION],
        }
    }
}
```

**Acceptance Criteria:**
- [ ] `Handshake`, `HandshakeAck`, `HandshakeError` serialize/deserialize to/from JSON correctly
- [ ] `PROTOCOL_VERSION` is 1
- [ ] `msg_type` field serializes as `"type"` in JSON (serde rename)
- [ ] Round-trip test: `serialize → deserialize` produces identical struct

---

### Unit 3: Protocol — Length-Prefixed JSON Codec

**File:** `crates/spectator-protocol/src/codec.rs`

```rust
use serde::{de::DeserializeOwned, Serialize};
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
```

**File:** `crates/spectator-protocol/src/codec.rs` — async variants for tokio

```rust
/// Async codec functions for use with tokio::net::TcpStream.
/// These use tokio::io::{AsyncReadExt, AsyncWriteExt}.
pub mod async_io {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Read one length-prefixed JSON message from an async reader.
    pub async fn read_message<T: DeserializeOwned>(
        reader: &mut (impl AsyncReadExt + Unpin),
    ) -> Result<T, CodecError> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes).await.map_err(CodecError::Io)?;
        let len = u32::from_be_bytes(len_bytes);
        if len > MAX_MESSAGE_SIZE {
            return Err(CodecError::MessageTooLarge(len));
        }
        let mut payload = vec![0u8; len as usize];
        reader.read_exact(&mut payload).await.map_err(CodecError::Io)?;
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
```

**Implementation Notes:**
- The codec module provides both sync (for `spectator-godot` which uses `std::net`) and async (for `spectator-server` which uses tokio) variants
- `spectator-protocol` gets a `tokio` dependency behind a feature flag: `tokio = { version = "1", features = ["io-util"], optional = true }` with `features = ["async"]` enabling it
- `spectator-server` depends on `spectator-protocol = { workspace = true, features = ["async"] }`
- `spectator-godot` depends on `spectator-protocol = { workspace = true }` (no async)

Update `crates/spectator-protocol/Cargo.toml`:

```toml
[package]
name = "spectator-protocol"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
tokio = { version = "1", features = ["io-util"], optional = true }

[features]
default = []
async = ["tokio"]
```

**Acceptance Criteria:**
- [ ] `encode()` produces `[4-byte BE length][JSON]`
- [ ] `read_message()` correctly reads a length-prefixed message
- [ ] `write_message()` writes a complete length-prefixed message
- [ ] Messages exceeding 16 MiB are rejected with `MessageTooLarge`
- [ ] Round-trip test: `write_message → read_message` produces identical struct
- [ ] Async variants compile and work with tokio `TcpStream`

---

### Unit 4: Protocol — Request/Response Message Envelope

**File:** `crates/spectator-protocol/src/messages.rs`

For M0, only the top-level message envelope is needed. Query methods are added in M1.

```rust
use serde::{Deserialize, Serialize};

/// Top-level message type tag, used to dispatch incoming messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Message {
    /// Addon → Server: initial handshake (sent unsolicited on connect)
    #[serde(rename = "handshake")]
    Handshake(crate::handshake::Handshake),

    /// Server → Addon: handshake accepted
    #[serde(rename = "handshake_ack")]
    HandshakeAck(crate::handshake::HandshakeAck),

    /// Server → Addon: handshake rejected
    #[serde(rename = "handshake_error")]
    HandshakeError(crate::handshake::HandshakeError),

    /// Server → Addon: query request
    #[serde(rename = "query")]
    Query {
        id: String,
        method: String,
        #[serde(default)]
        params: serde_json::Value,
    },

    /// Addon → Server: query response
    #[serde(rename = "response")]
    Response {
        id: String,
        data: serde_json::Value,
    },

    /// Addon → Server: query error
    #[serde(rename = "error")]
    Error {
        id: String,
        code: String,
        message: String,
    },

    /// Addon → Server: push event (unsolicited)
    #[serde(rename = "event")]
    Event {
        event: String,
        #[serde(flatten)]
        data: serde_json::Value,
    },
}
```

**Implementation Notes:**
- Uses `#[serde(tag = "type")]` for internally tagged enum — the JSON `"type"` field determines which variant
- The `Handshake` variant's inner struct also has a `msg_type` field set to `"handshake"` — this creates a conflict with `#[serde(tag = "type")]`. **Resolution:** Remove the `msg_type` field from `Handshake`, `HandshakeAck`, and `HandshakeError` structs. The `#[serde(tag = "type")]` on the `Message` enum handles the type discriminator. The individual structs should NOT have their own `type` field.
- For M0, only `Handshake`, `HandshakeAck`, and `HandshakeError` are used. The `Query`/`Response`/`Error`/`Event` variants are defined but not dispatched until M1.

**Revised handshake structs (remove `msg_type` field):**

```rust
// In handshake.rs — REVISED: no msg_type field (handled by Message enum tag)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Handshake {
    pub spectator_version: String,
    pub protocol_version: u32,
    pub godot_version: String,
    pub scene_dimensions: u32,
    pub physics_ticks_per_sec: u32,
    pub project_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeAck {
    pub spectator_version: String,
    pub protocol_version: u32,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeError {
    pub message: String,
    pub server_version: String,
    pub supported_protocols: Vec<u32>,
}
```

This way, `Message::Handshake(h)` serializes to:
```json
{"type": "handshake", "spectator_version": "0.1.0", "protocol_version": 1, ...}
```

**Acceptance Criteria:**
- [ ] `Message::Handshake(h)` serializes with `"type": "handshake"` field
- [ ] `Message::HandshakeAck(a)` serializes with `"type": "handshake_ack"` field
- [ ] Deserialization of `{"type": "handshake", ...}` produces `Message::Handshake(_)`
- [ ] `Query`, `Response`, `Error`, `Event` variants serialize/deserialize correctly
- [ ] Round-trip tests pass for all variants

---

### Unit 5: GDExtension — `SpectatorTCPServer` Class

**File:** `crates/spectator-godot/src/tcp_server.rs`

This is the core networking class for M0. It manages a TCP listener that accepts a single connection and handles the handshake.

```rust
use godot::prelude::*;
use spectator_protocol::{
    codec,
    handshake::{Handshake, PROTOCOL_VERSION},
    messages::Message,
};
use std::io::{self, ErrorKind};
use std::net::{TcpListener, TcpStream};

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorTCPServer {
    base: Base<Node>,
    listener: Option<TcpListener>,
    client: Option<TcpStream>,
    port: i32,
    handshake_completed: bool,
}

#[godot_api]
impl INode for SpectatorTCPServer {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            listener: None,
            client: None,
            port: 9077,
            handshake_completed: false,
        }
    }
}

#[godot_api]
impl SpectatorTCPServer {
    /// Start listening on the given port. Binds to localhost only.
    #[func]
    pub fn start(&mut self, port: i32) {
        self.port = port;
        let addr = format!("127.0.0.1:{}", port);
        match TcpListener::bind(&addr) {
            Ok(listener) => {
                listener.set_nonblocking(true).ok();
                self.listener = Some(listener);
                godot_print!("[Spectator] TCP server listening on {}", addr);
            }
            Err(e) => {
                godot_error!("[Spectator] Failed to bind to {}: {}", addr, e);
            }
        }
    }

    /// Stop listening and close any active connection.
    #[func]
    pub fn stop(&mut self) {
        self.client = None;
        self.listener = None;
        self.handshake_completed = false;
        godot_print!("[Spectator] TCP server stopped");
    }

    /// Returns true if a client is connected and handshake is complete.
    #[func]
    pub fn is_connected(&self) -> bool {
        self.handshake_completed
    }

    /// Poll for new connections and incoming messages. Call every _physics_process.
    #[func]
    pub fn poll(&mut self) {
        // Accept new connections if we don't have one
        if self.client.is_none() {
            self.try_accept();
        }

        // Read incoming messages from connected client
        if self.client.is_some() {
            self.try_read();
        }
    }
}

// Private implementation methods (not exposed to GDScript)
impl SpectatorTCPServer {
    fn try_accept(&mut self) {
        let listener = match &self.listener {
            Some(l) => l,
            None => return,
        };

        match listener.accept() {
            Ok((stream, addr)) => {
                godot_print!("[Spectator] Client connected from {}", addr);
                stream.set_nonblocking(true).ok();
                self.client = Some(stream);
                self.handshake_completed = false;
                self.send_handshake();
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // No pending connection — normal for non-blocking
            }
            Err(e) => {
                godot_error!("[Spectator] Accept error: {}", e);
            }
        }
    }

    fn send_handshake(&mut self) {
        let handshake = Handshake::new(
            self.get_godot_version(),
            self.detect_scene_dimensions(),
            self.get_physics_ticks(),
            self.get_project_name(),
        );
        let msg = Message::Handshake(handshake);

        if let Some(ref mut stream) = &mut self.client {
            // Temporarily set blocking for the handshake write
            stream.set_nonblocking(false).ok();
            match codec::write_message(stream, &msg) {
                Ok(()) => {
                    godot_print!("[Spectator] Handshake sent");
                }
                Err(e) => {
                    godot_error!("[Spectator] Failed to send handshake: {}", e);
                    self.disconnect_client();
                }
            }
            if let Some(ref stream) = &self.client {
                stream.set_nonblocking(true).ok();
            }
        }
    }

    fn try_read(&mut self) {
        let stream = match &mut self.client {
            Some(s) => s,
            None => return,
        };

        // Temporarily set blocking with a very short timeout for reads
        stream.set_nonblocking(false).ok();
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(1)))
            .ok();

        match codec::read_message::<Message>(stream) {
            Ok(msg) => {
                self.handle_message(msg);
            }
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut =>
            {
                // No data available — normal
            }
            Err(codec::CodecError::Io(ref e))
                if e.kind() == ErrorKind::UnexpectedEof
                    || e.kind() == ErrorKind::ConnectionReset =>
            {
                godot_print!("[Spectator] Client disconnected");
                self.disconnect_client();
                return;
            }
            Err(e) => {
                godot_error!("[Spectator] Read error: {}", e);
                self.disconnect_client();
                return;
            }
        }

        // Restore non-blocking
        if let Some(ref stream) = &self.client {
            stream.set_nonblocking(true).ok();
        }
    }

    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::HandshakeAck(ack) => {
                godot_print!(
                    "[Spectator] Handshake ACK received — session {}",
                    ack.session_id
                );
                self.handshake_completed = true;
            }
            Message::HandshakeError(err) => {
                godot_error!("[Spectator] Handshake rejected: {}", err.message);
                self.disconnect_client();
            }
            _ => {
                // M0: ignore other message types
                godot_print!("[Spectator] Received message (unhandled in M0)");
            }
        }
    }

    fn disconnect_client(&mut self) {
        self.client = None;
        self.handshake_completed = false;
    }

    fn get_godot_version(&self) -> String {
        let info = godot::classes::Engine::singleton().get_version_info();
        let major = info.get("major").unwrap_or(Variant::from(0)).to::<i32>();
        let minor = info.get("minor").unwrap_or(Variant::from(0)).to::<i32>();
        format!("{}.{}", major, minor)
    }

    fn detect_scene_dimensions(&self) -> u32 {
        // M0: default to 3. Full detection in M9 (2D support).
        3
    }

    fn get_physics_ticks(&self) -> u32 {
        godot::classes::Engine::singleton()
            .get_physics_ticks_per_second() as u32
    }

    fn get_project_name(&self) -> String {
        godot::classes::ProjectSettings::singleton()
            .get_setting("application/config/name".into())
            .to::<GString>()
            .to_string()
    }
}
```

**File:** `crates/spectator-godot/src/lib.rs` (updated to register modules)

```rust
use godot::prelude::*;

mod tcp_server;

struct SpectatorExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SpectatorExtension {}
```

**Implementation Notes:**
- Uses `std::net::TcpListener` and `TcpStream` (not tokio) because the GDExtension runs on Godot's main thread with no async runtime
- Non-blocking mode with a 1ms read timeout to avoid blocking `_physics_process`
- Temporarily switches to blocking for the handshake write (small message, instant on localhost)
- Scene dimension detection is hardcoded to 3 for M0; will be implemented in M9

**Acceptance Criteria:**
- [ ] `SpectatorTCPServer` class is available in GDScript after GDExtension loads
- [ ] `start(port)` binds to `127.0.0.1:{port}` and logs success
- [ ] `start()` with an in-use port logs a clear error, does not crash
- [ ] `poll()` accepts incoming connections without blocking
- [ ] On connection, sends a `Handshake` message with version, protocol, Godot version, project name
- [ ] Receives `HandshakeAck` and sets `is_connected()` to true
- [ ] Receives `HandshakeError` and disconnects
- [ ] Detects client disconnect (EOF/ConnectionReset) and resets state
- [ ] `stop()` closes listener and client

---

### Unit 6: MCP Server — TCP Client with Reconnection

**File:** `crates/spectator-server/src/tcp.rs`

```rust
use anyhow::Result;
use spectator_protocol::{
    codec::async_io,
    handshake::{HandshakeAck, HandshakeError, PROTOCOL_VERSION},
    messages::Message,
};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

/// Handle to the TCP connection's write half, for sending queries.
pub struct TcpClientHandle {
    pub writer: WriteHalf<TcpStream>,
}

/// Shared state between MCP handlers and TCP client task.
pub struct SessionState {
    pub tcp_writer: Option<TcpClientHandle>,
    pub connected: bool,
    pub session_id: Option<String>,
    pub handshake_info: Option<HandshakeInfo>,
}

/// Information received from the addon during handshake.
#[derive(Debug, Clone)]
pub struct HandshakeInfo {
    pub spectator_version: String,
    pub godot_version: String,
    pub scene_dimensions: u32,
    pub physics_ticks_per_sec: u32,
    pub project_name: String,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            tcp_writer: None,
            connected: false,
            session_id: None,
            handshake_info: None,
        }
    }
}

/// Background task: connect to addon, handle handshake, reconnect on disconnect.
pub async fn tcp_client_loop(state: Arc<Mutex<SessionState>>, port: u16) {
    loop {
        tracing::info!("Connecting to Godot addon on 127.0.0.1:{}...", port);

        match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
            Ok(stream) => {
                tracing::info!("Connected to addon");

                match handle_connection(stream, state.clone()).await {
                    Ok(()) => tracing::info!("Connection closed normally"),
                    Err(e) => tracing::warn!("Connection error: {}", e),
                }

                // Clean up state on disconnect
                {
                    let mut s = state.lock().await;
                    s.tcp_writer = None;
                    s.connected = false;
                }

                tracing::info!("Addon disconnected, will retry in 2s");
            }
            Err(e) => {
                tracing::debug!("Connection failed: {}", e);
            }
        }

        sleep(Duration::from_secs(2)).await;
    }
}

async fn handle_connection(
    stream: TcpStream,
    state: Arc<Mutex<SessionState>>,
) -> Result<()> {
    let (mut reader, writer) = tokio::io::split(stream);

    // Step 1: Read handshake from addon
    let msg: Message = async_io::read_message(&mut reader).await
        .map_err(|e| anyhow::anyhow!("Failed to read handshake: {}", e))?;

    let handshake = match msg {
        Message::Handshake(h) => h,
        other => {
            anyhow::bail!("Expected handshake, got: {:?}", other);
        }
    };

    tracing::info!(
        "Handshake received: project={}, godot={}, dimensions={}D, protocol=v{}",
        handshake.project_name,
        handshake.godot_version,
        handshake.scene_dimensions,
        handshake.protocol_version,
    );

    // Step 2: Validate protocol version
    if handshake.protocol_version != PROTOCOL_VERSION {
        let error = HandshakeError::version_mismatch(handshake.protocol_version);
        let mut writer = writer;
        async_io::write_message(&mut writer, &Message::HandshakeError(error)).await
            .map_err(|e| anyhow::anyhow!("Failed to send handshake error: {}", e))?;
        anyhow::bail!(
            "Protocol version mismatch: expected {}, got {}",
            PROTOCOL_VERSION,
            handshake.protocol_version
        );
    }

    // Step 3: Send ACK
    let session_id = format!("sess_{}", Uuid::new_v4().as_simple());
    let ack = HandshakeAck::new(session_id.clone());
    let mut writer = writer;
    async_io::write_message(&mut writer, &Message::HandshakeAck(ack)).await
        .map_err(|e| anyhow::anyhow!("Failed to send handshake ACK: {}", e))?;

    tracing::info!("Handshake complete — session {}", session_id);

    // Step 4: Update shared state
    {
        let mut s = state.lock().await;
        s.tcp_writer = Some(TcpClientHandle { writer });
        s.connected = true;
        s.session_id = Some(session_id);
        s.handshake_info = Some(HandshakeInfo {
            spectator_version: handshake.spectator_version,
            godot_version: handshake.godot_version,
            scene_dimensions: handshake.scene_dimensions,
            physics_ticks_per_sec: handshake.physics_ticks_per_sec,
            project_name: handshake.project_name,
        });
    }

    // Step 5: Read loop — process incoming messages until disconnect
    loop {
        match async_io::read_message::<Message>(&mut reader).await {
            Ok(msg) => {
                // M0: just log. M1+ will dispatch to handlers.
                tracing::debug!("Received message from addon: {:?}", msg);
            }
            Err(e) => {
                tracing::debug!("Read error (likely disconnect): {}", e);
                break;
            }
        }
    }

    Ok(())
}
```

**Implementation Notes:**
- The `tcp_client_loop` runs as a `tokio::spawn`ed background task
- Uses `tokio::io::split` to separate read/write halves — write half goes in `SessionState` for MCP handlers to send queries in M1
- On disconnect, the loop sleeps 2s and retries
- The session ID uses `uuid::v4` with a `sess_` prefix for readability
- `tracing` is used for all logging (writes to stderr, keeps stdout clean for MCP)

**Acceptance Criteria:**
- [ ] Server connects to `127.0.0.1:9077` when addon is listening
- [ ] Server reads handshake from addon, logs project name, Godot version, dimensions
- [ ] Server sends `HandshakeAck` with session ID
- [ ] Protocol version mismatch → server sends `HandshakeError` and disconnects
- [ ] On disconnect, server retries every 2 seconds
- [ ] `SessionState.connected` is `true` after successful handshake, `false` after disconnect
- [ ] All logging goes to stderr (no stdout corruption of MCP transport)

---

### Unit 7: MCP Server — Stdio MCP Skeleton

**File:** `crates/spectator-server/src/server.rs`

```rust
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::handler::server::ServerHandler;
use rmcp::tool_box;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tcp::SessionState;

#[derive(Clone)]
pub struct SpectatorServer {
    pub state: Arc<Mutex<SessionState>>,
}

impl SpectatorServer {
    pub fn new(state: Arc<Mutex<SessionState>>) -> Self {
        Self { state }
    }
}

#[tool_box]
impl SpectatorServer {
    // No tools defined in M0. The #[tool_box] macro is present to establish
    // the pattern. Tools are added in M1+.
}

impl ServerHandler for SpectatorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "spectator-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}
```

**File:** `crates/spectator-server/src/main.rs` (full M0 version)

```rust
mod server;
mod tcp;

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use std::sync::Arc;
use tokio::sync::Mutex;

use server::SpectatorServer;
use tcp::SessionState;

/// Default TCP port for connecting to the Godot addon.
const DEFAULT_PORT: u16 = 9077;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to stderr (stdout is MCP protocol only)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("spectator=info".parse()?),
        )
        .init();

    tracing::info!("spectator-server v{}", env!("CARGO_PKG_VERSION"));

    // Parse port from env or use default
    let port: u16 = std::env::var("SPECTATOR_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // Shared state between MCP handlers and TCP client
    let state = Arc::new(Mutex::new(SessionState::default()));

    // Spawn TCP client background task (reconnects automatically)
    let tcp_state = state.clone();
    tokio::spawn(async move {
        tcp::tcp_client_loop(tcp_state, port).await;
    });

    // Start MCP server on stdio — blocks until AI client disconnects
    let server = SpectatorServer::new(state);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    tracing::info!("MCP session ended, shutting down");
    Ok(())
}
```

**Implementation Notes:**
- Port is configurable via `SPECTATOR_PORT` environment variable
- The `#[tool_box]` macro with no tools still generates valid `list_tools` (empty list) and `call_tool` (returns error for any call) — this is fine for M0
- `ServiceExt::serve` starts the MCP JSON-RPC handler on stdin/stdout
- TCP client is spawned before `.serve()` so it starts connecting immediately

**Acceptance Criteria:**
- [ ] `spectator-server` starts, logs version to stderr
- [ ] MCP handshake completes on stdio (AI client sees server info with name "spectator-server")
- [ ] `tools/list` returns empty tool list
- [ ] TCP client loop runs concurrently with MCP handler
- [ ] `SPECTATOR_PORT=9078` makes server connect to port 9078 instead of 9077
- [ ] Process exits cleanly when stdin closes (AI client disconnects)

---

### Unit 8: GDExtension Manifest

**File:** `addons/spectator/spectator.gdextension`

```ini
[configuration]
entry_symbol = "gdext_rust_init"
compatibility_minimum = "4.2"
reloadable = true

[libraries]
linux.debug.x86_64 = "res://addons/spectator/bin/linux/libspectator_godot.so"
linux.release.x86_64 = "res://addons/spectator/bin/linux/libspectator_godot.so"
```

**Implementation Notes:**
- `entry_symbol = "gdext_rust_init"` must match what gdext's `#[gdextension]` macro exports
- `compatibility_minimum = "4.2"` ensures the addon won't load in older Godot versions
- `reloadable = true` enables hot-reload when recompiling during development
- Only Linux debug target for M0. macOS and Windows targets added in M11 (Distribution)
- Both debug and release point to the same `.so` for M0 simplicity. CI can produce separate debug/release builds later.

**Acceptance Criteria:**
- [ ] Godot 4.2+ recognizes the `.gdextension` file when present in the project
- [ ] GDExtension loads without errors when the `.so` binary is present
- [ ] `SpectatorTCPServer` class is available in GDScript after loading
- [ ] Error logged (not crash) if binary is missing for the current platform

---

### Unit 9: GDScript Plugin

**File:** `addons/spectator/plugin.cfg`

```ini
[plugin]
name="Spectator"
description="Spatial debugging for AI agents — gives your AI eyes into the running game"
author="Spectator Contributors"
version="0.1.0"
script="plugin.gd"
```

**File:** `addons/spectator/plugin.gd`

```gdscript
@tool
extends EditorPlugin


func _enable_plugin() -> void:
    add_autoload_singleton("SpectatorRuntime", "res://addons/spectator/runtime.gd")


func _disable_plugin() -> void:
    remove_autoload_singleton("SpectatorRuntime")
```

**Implementation Notes:**
- Uses `_enable_plugin` / `_disable_plugin` (NOT `_enter_tree` / `_exit_tree`) to avoid the autoload timing bug documented in the godot-addon skill
- M0 has no dock panel — that's M6. The plugin only manages the autoload registration.
- The autoload name `SpectatorRuntime` is the global singleton name accessible via `get_node("/root/SpectatorRuntime")`

**Acceptance Criteria:**
- [ ] Enabling plugin in Project Settings → Plugins → Spectator registers the autoload without errors
- [ ] Disabling plugin removes the autoload
- [ ] No errors in Output panel during enable/disable
- [ ] Re-enabling after disable works correctly (no duplicate autoloads)

---

### Unit 10: GDScript Runtime Autoload

**File:** `addons/spectator/runtime.gd`

```gdscript
extends Node

var tcp_server: SpectatorTCPServer


func _ready() -> void:
    # Check that GDExtension classes are available
    if not ClassDB.class_exists(&"SpectatorTCPServer"):
        push_error("[Spectator] GDExtension not loaded — SpectatorTCPServer class not found. Check that the spectator.gdextension binary exists for your platform.")
        return

    tcp_server = SpectatorTCPServer.new()
    add_child(tcp_server)

    var port: int = ProjectSettings.get_setting("spectator/connection/port", 9077)
    tcp_server.start(port)


func _physics_process(_delta: float) -> void:
    if tcp_server:
        tcp_server.poll()


func _exit_tree() -> void:
    if tcp_server:
        tcp_server.stop()
```

**Implementation Notes:**
- Checks `ClassDB.class_exists()` before instantiating GDExtension classes to provide a clear error if the binary is missing
- Port is read from Project Settings with a fallback default of 9077
- `poll()` is called every `_physics_process` frame (typically 60fps) to check for connections and messages
- `_exit_tree` ensures the TCP server is stopped when the game exits

**Acceptance Criteria:**
- [ ] Autoload initializes when game starts (Play button in editor)
- [ ] `SpectatorTCPServer` is created and starts listening on the configured port
- [ ] `poll()` is called every physics frame
- [ ] Clear error message if GDExtension binary is missing
- [ ] TCP server is stopped when game stops

---

### Unit 11: CLAUDE.md

**File:** `CLAUDE.md`

```markdown
# Spectator — Agent Instructions

## What This Is

Spectator: Rust MCP server + Rust GDExtension addon giving AI agents spatial
awareness of running Godot games. Two Rust compilation targets that communicate
over TCP.

## Repository Layout

```
crates/
  spectator-server/     — MCP binary (rmcp + tokio), stdio transport
  spectator-godot/      — GDExtension cdylib (gdext), loaded by Godot
  spectator-protocol/   — Shared TCP wire format types
  spectator-core/       — Shared spatial logic (no Godot, no MCP)
addons/spectator/       — Godot addon (GDScript + GDExtension manifest)
docs/                   — Design documents
docs/design/            — Implementation designs per milestone
```

## Build Commands

```bash
# Build everything
cargo build --workspace

# Build specific crate
cargo build -p spectator-server
cargo build -p spectator-godot

# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Copy GDExtension to addon (Linux)
cp target/debug/libspectator_godot.so addons/spectator/bin/linux/
```

## Key Constraints

- **stdout is sacred**: spectator-server uses stdout for MCP protocol. ALL
  logging goes to stderr via `tracing` / `eprintln!`. Never use `println!`.
- **Main thread only**: spectator-godot runs on Godot's main thread. No
  `Gd<T>` across thread boundaries. All scene tree access in _physics_process.
- **GDExtension ≠ EditorPlugin**: GDExtension classes can't be EditorPlugin
  bases (godot#85268). GDScript `plugin.gd` is the EditorPlugin; Rust classes
  are instantiated by it.
- **Thin addon**: GDExtension answers "what does the engine say?" The server
  does all spatial reasoning, budgeting, diffing, indexing.

## Code Style

- Rust edition 2024, workspace versioning
- `tracing` for all logging (never `println!`, use `eprintln!` only for
  one-off debugging)
- `anyhow` for application errors in spectator-server
- `thiserror` or manual `impl Error` for library errors in protocol/core
- serde for all serialization, `#[serde(rename_all = "snake_case")]` for enums
- Tests alongside source in `#[cfg(test)] mod tests`
- No unwrap in library code; unwrap OK in tests and main.rs setup

## Architecture Rules

- spectator-godot depends on spectator-protocol, NOT on spectator-core
- spectator-server depends on both spectator-protocol and spectator-core
- spectator-core has zero Godot or MCP dependencies — pure logic
- TCP protocol: length-prefixed JSON (4-byte BE u32 + JSON payload)
- Addon listens (port 9077), server connects (ephemeral)
```

**Acceptance Criteria:**
- [ ] `CLAUDE.md` exists at repo root
- [ ] Contains build commands, architecture rules, code style, key constraints
- [ ] Accurately reflects the actual repo structure and conventions

---

### Unit 12: CI Workflow

**File:** `.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-Dwarnings"

jobs:
  check:
    name: Check, Lint, Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo registry and build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Check formatting
        run: cargo fmt --check --all

      - name: Clippy
        run: cargo clippy --workspace --all-targets

      - name: Run tests
        run: cargo test --workspace

  build:
    name: Build Release
    runs-on: ubuntu-latest
    needs: check
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry and build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-release-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-release-

      - name: Build server (release)
        run: cargo build --release -p spectator-server

      - name: Build GDExtension (release)
        run: cargo build --release -p spectator-godot
```

**Implementation Notes:**
- Two jobs: `check` (fmt + clippy + test) runs first, `build` (release binaries) only if checks pass
- `RUSTFLAGS: "-Dwarnings"` makes clippy warnings into errors
- Cargo caching speeds up builds significantly
- gdext's build script downloads Godot API headers automatically — no Godot installation needed for compilation

**Acceptance Criteria:**
- [ ] `cargo fmt --check --all` passes
- [ ] `cargo clippy --workspace --all-targets` passes with no warnings
- [ ] `cargo test --workspace` passes
- [ ] `cargo build --release -p spectator-server` produces binary
- [ ] `cargo build --release -p spectator-godot` produces `.so`

---

### Unit 13: MCP Configuration Example

**File:** `.mcp.json`

```json
{
  "mcpServers": {
    "spectator": {
      "type": "stdio",
      "command": "./target/release/spectator-server",
      "env": {
        "SPECTATOR_PORT": "9077"
      }
    }
  }
}
```

**Implementation Notes:**
- This is the Claude Code MCP configuration format
- Users copy this and adjust the command path to wherever they installed the binary
- `SPECTATOR_PORT` env var is optional (defaults to 9077)

**Acceptance Criteria:**
- [ ] Claude Code recognizes the MCP config and spawns `spectator-server`
- [ ] Server appears in the MCP server list with name "spectator-server"

---

### Unit 14: Directory Structure & Binary Copy Script

Create the addon binary directory and a convenience script:

**Directories to create:**
- `addons/spectator/bin/linux/`

**File:** `scripts/copy-gdext.sh`

```bash
#!/usr/bin/env bash
# Copy the built GDExtension library to the addon directory.
# Usage: ./scripts/copy-gdext.sh [debug|release]

set -euo pipefail

MODE="${1:-debug}"
SRC="target/${MODE}/libspectator_godot.so"
DST="addons/spectator/bin/linux/"

if [ ! -f "$SRC" ]; then
    echo "Error: $SRC not found. Run 'cargo build -p spectator-godot' first."
    exit 1
fi

mkdir -p "$DST"
cp "$SRC" "$DST"
echo "Copied $SRC → $DST"
```

**Acceptance Criteria:**
- [ ] `addons/spectator/bin/linux/` directory exists
- [ ] `scripts/copy-gdext.sh` copies the built library to the correct location
- [ ] Script reports an error if the library hasn't been built yet

---

## Implementation Order

Dependencies flow top-down; implement in this order:

1. **Unit 1: Cargo Workspace** — everything depends on this compiling
2. **Unit 2: Handshake Types** — needed by both TCP endpoints
3. **Unit 3: Length-Prefixed Codec** — needed by both TCP endpoints
4. **Unit 4: Message Envelope** — needed by both TCP endpoints
5. **Unit 8: GDExtension Manifest** — needed before GDScript can reference Rust classes
6. **Unit 5: SpectatorTCPServer** — the addon-side TCP implementation
7. **Unit 14: Directory Structure** — needed to place the built binary
8. **Unit 9: Plugin GDScript** — depends on GDExtension being loadable
9. **Unit 10: Runtime GDScript** — depends on plugin + GDExtension classes
10. **Unit 6: TCP Client** — server-side TCP, depends on protocol types
11. **Unit 7: MCP Server** — depends on TCP client + state types
12. **Unit 11: CLAUDE.md** — can be done anytime, but best after structure is final
13. **Unit 12: CI** — validates everything compiles and passes
14. **Unit 13: MCP Config** — last, after server binary works

**Parallelizable:** Units 2-4 (protocol) can be done together. Units 8-10 (addon) can be done together after protocol. Units 6-7 (server) can be done together after protocol.

---

## Testing

### Unit Tests: `crates/spectator-protocol/src/handshake.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::Message;

    #[test]
    fn handshake_round_trip() {
        let h = Handshake::new("4.3".into(), 3, 60, "TestProject".into());
        let msg = Message::Handshake(h.clone());
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Message::Handshake(ref inner) if inner == &h));
    }

    #[test]
    fn handshake_has_type_tag() {
        let h = Handshake::new("4.3".into(), 3, 60, "TestProject".into());
        let msg = Message::Handshake(h);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"handshake""#));
    }

    #[test]
    fn handshake_ack_round_trip() {
        let ack = HandshakeAck::new("sess_abc123".into());
        let msg = Message::HandshakeAck(ack.clone());
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Message::HandshakeAck(ref inner) if inner == &ack));
    }

    #[test]
    fn handshake_error_round_trip() {
        let err = HandshakeError::version_mismatch(99);
        let msg = Message::HandshakeError(err.clone());
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Message::HandshakeError(ref inner) if inner == &err));
    }
}
```

### Unit Tests: `crates/spectator-protocol/src/codec.rs`

```rust
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
```

### Integration Test Approach

No automated Godot integration tests in M0 (requires a running Godot instance). Manual verification via the exit criteria:

1. Build: `cargo build --workspace`
2. Copy GDExtension: `./scripts/copy-gdext.sh debug`
3. Open a Godot 4.2+ project with `addons/spectator/` copied in
4. Enable plugin in Project Settings → Plugins
5. Press Play — check Output panel for `[Spectator] TCP server listening on 127.0.0.1:9077`
6. Run: `cargo run -p spectator-server` in a terminal
7. Verify server logs: `Connected to addon`, `Handshake received`, `Handshake complete`
8. Verify Godot Output: `[Spectator] Client connected`, `[Spectator] Handshake ACK received`
9. Stop game in Godot — verify server logs: `Addon disconnected, will retry in 2s`
10. Press Play again — verify server reconnects and handshake succeeds again

---

## Verification Checklist

```bash
# 1. Everything compiles
cargo build --workspace

# 2. No lint warnings
cargo clippy --workspace --all-targets

# 3. Formatting correct
cargo fmt --check --all

# 4. Tests pass
cargo test --workspace

# 5. Server binary runs
cargo run -p spectator-server 2>&1 | head -3
# Should show: spectator-server v0.1.0, then connection attempts

# 6. GDExtension produces shared library
ls target/debug/libspectator_godot.so

# 7. Addon structure is correct
ls addons/spectator/plugin.cfg addons/spectator/plugin.gd \
   addons/spectator/runtime.gd addons/spectator/spectator.gdextension

# 8. CLAUDE.md exists
test -f CLAUDE.md && echo "OK"

# 9. CI config exists
test -f .github/workflows/ci.yml && echo "OK"

# 10. MCP config example exists
test -f .mcp.json && echo "OK"
```
