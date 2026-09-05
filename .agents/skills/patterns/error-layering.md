# Pattern: Three-Tier Error Layering

Errors use three distinct types at three distinct layers: `CodecError` (library/protocol), `anyhow::Result` (server startup/background tasks), and `McpError` (MCP tool handlers). Each layer converts from the layer below using the `?` operator or explicit mapping.

## Rationale
Keeps library crates dependency-free from MCP concerns. Server startup uses `anyhow` for ergonomic error propagation. Tool handlers must return `McpError` (rmcp SDK requirement). Conversions are explicit and localized.

## Examples

### Example 1: Library layer — custom error type with Display + Error impl
**File**: `crates/stage-protocol/src/codec.rs:48-77`
```rust
#[derive(Debug)]
pub enum CodecError {
    Io(io::Error),
    Serialize(serde_json::Error),
    Deserialize(serde_json::Error),
    MessageTooLarge(u32),
}

impl std::fmt::Display for CodecError { ... }
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

### Example 2: Background task layer — anyhow::Result for ergonomic propagation
**File**: `crates/stage-server/src/tcp.rs:123` and `crates/stage-server/src/main.rs:19`
```rust
// main.rs
#[tokio::main]
async fn main() -> Result<()> { ... }  // anyhow::Result

// tcp.rs
async fn handle_connection(stream: TcpStream, state: Arc<Mutex<SessionState>>) -> Result<()> {
    let msg: Message = async_io::read_message(&mut reader)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read handshake: {}", e))?;
    ...
}
```

### Example 3: MCP handler layer — McpError with inline construction
**File**: `crates/stage-server/src/mcp/mod.rs:32-44`
```rust
fn serialize_params<T: Serialize>(params: &T) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(params).map_err(|e| {
        McpError::internal_error(format!("Param serialization error: {e}"), None)
    })
}

fn deserialize_response<T: for<'de> Deserialize<'de>>(data: serde_json::Value) -> Result<T, McpError> {
    serde_json::from_value(data).map_err(|e| {
        McpError::internal_error(format!("Response deserialization error: {e}"), None)
    })
}
```

### Example 4: Handler-layer parameter validation
**File**: `crates/stage-server/src/mcp/query.rs` — validation returns `Result<QueryOrigin, McpError>` with `McpError::invalid_params(msg, None)` for user input errors.

## When to Use
- New library crate error type: implement `Display` + `std::error::Error` manually (use `thiserror` is acceptable too)
- Background task or `main()`: use `anyhow::Result`
- MCP tool handler: return `Result<String, McpError>`; convert using `McpError::internal_error` or `McpError::invalid_params`

## When NOT to Use
- `.unwrap()` in library code — only acceptable in tests and `main.rs` setup
- `anyhow` in MCP handlers — must use `McpError` at the handler boundary

## Common Violations
- Propagating `anyhow::Error` into an MCP handler — must convert at the boundary
- Using `McpError::internal_error` for user input validation errors — use `McpError::invalid_params` instead
- `thiserror` in library crates that should remain dependency-light — use manual `impl` or add `thiserror` as a dev dependency
