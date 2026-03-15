---
name: rmcp
description: Working with the rmcp Rust MCP SDK in the stage-server crate. Use when writing or modifying MCP tool definitions, server initialization, or tool call handling.
---

# rmcp — Rust MCP Server SDK

This skill covers the `rmcp` crate used in `crates/stage-server`. The MCP server exposes Stage's 9 tools to AI agents via stdio transport.

## Cargo.toml

```toml
[dependencies]
rmcp = { version = "0.16", features = ["server"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
schemars = "1"
anyhow = "1"
```

## Server Initialization — `main()`

```rust
use rmcp::{transport::stdio, ServiceExt};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Build server with shared state
    let server = StageServer::new().await?;

    // Spawn background TCP client task BEFORE blocking on MCP
    let tcp_state = server.state.clone();
    tokio::spawn(async move {
        tcp_client_loop(tcp_state).await;
    });

    // Start MCP server on stdio — this blocks until client disconnects
    server.serve(stdio()).await?.waiting().await?;

    Ok(())
}
```

**Key point:** `tokio::spawn` background tasks before calling `.waiting().await`. The MCP stdio handler blocks the current task — anything you need to run concurrently must be spawned first.

**stderr for logging:** stdout is MCP protocol only. Use `eprintln!` or a logger configured to write to stderr.

## Defining Tools — The `#[tool_router]` Pattern

Tools live on a struct that derives `Clone` (required for shared state pattern):

```rust
#[derive(Clone)]
pub struct StageServer {
    pub state: Arc<Mutex<SessionState>>,
}

#[tool_router]
impl StageServer {
    #[tool(description = "Get a spatial snapshot of the current scene from a perspective")]
    async fn spatial_snapshot(
        &self,
        params: SpatialSnapshotParams,
    ) -> Result<String, McpError> {
        let state = self.state.lock().await;
        // ... query addon, process, return JSON string
        Ok(serde_json::to_string(&response)?)
    }
}
```

`#[tool_router]` on the impl block auto-generates tool listing and routing. `#[tool(description = "...")]` on each method registers it as an MCP tool. The description is what the AI model sees — write it from the agent's perspective ("Get", "Returns", "Query").

## Parameter Structs

Every tool gets a dedicated params struct:

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialSnapshotParams {
    /// Where to look from. Defaults to active camera.
    #[serde(default)]
    pub perspective: PerspectiveMode,  // enum with #[default] Camera

    /// Node path when perspective is "node"
    pub focal_node: Option<String>,

    /// World position when perspective is "point"
    pub focal_point: Option<Vec<f64>>,

    /// Max distance from focal point
    #[serde(default = "default_radius")]
    pub radius: f64,

    /// Detail tier: summary, standard, or full
    #[serde(default)]
    pub detail: DetailLevel,  // enum with #[default] Standard

    pub groups: Option<Vec<String>>,
    pub class_filter: Option<Vec<String>>,

    #[serde(default)]
    pub include_offscreen: bool,

    pub token_budget: Option<u32>,
    pub expand: Option<String>,
}

// In defaults.rs:
fn default_radius() -> f64 { 50.0 }
// perspective and detail use #[derive(Default)] on their enum types
```

**Required derives:**
- `Deserialize` — params arrive as JSON from the AI client
- `JsonSchema` — generates schema the client uses for validation/documentation
- `Debug` — useful for logging
- Do NOT need `Serialize` on params structs (only on response types)

**`#[serde(default)]` vs `Option<T>`:**
- `Option<String>` = field is optional, will be `None` if not provided
- `#[serde(default = "fn_name")]` = field is optional with a non-None default value
- `#[serde(default)]` = uses `Default::default()` (e.g., `false` for bool, `0` for int)

## Return Types and Errors

Tools return `Result<String, McpError>` (string gets wrapped in TextContent automatically):

```rust
use rmcp::model::ErrorData as McpError;

// Success — return JSON string
Ok(serde_json::to_string(&response).map_err(|e| {
    McpError::internal_error(format!("serialization failed: {e}"), None)
})?)

// Structured error with code
Err(McpError::invalid_params("Node 'enemies/scout_99' not found", None))

// Internal error
Err(McpError::internal_error("TCP connection lost", None))
```

**Standard error constructors on `McpError`:**
- `McpError::invalid_params(message, data)` — bad agent input
- `McpError::internal_error(message, data)` — server/addon side failure
- For Stage's custom codes, use `McpError::new(code, message, data)` with our error code enum

**Distinguish agent errors from server errors:**
- Agent's fault (bad node path, invalid params) → `invalid_params` → agent can fix and retry
- Our fault (TCP drop, serialization fail) → `internal_error` → agent should report

## Implementing `ServerHandler`

`#[tool_router]` generates much of `ServerHandler` automatically, but you still implement:

```rust
impl ServerHandler for StageServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "stage-server".to_string(),
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

When using `#[tool_router]`, the `list_tools` and `call_tool` methods are generated automatically. You only need `get_info`.

## Shared State

```rust
#[derive(Clone)]
pub struct StageServer {
    pub state: Arc<Mutex<SessionState>>,  // tokio::sync::Mutex for async
}

pub struct SessionState {
    pub tcp_writer: Option<TcpClientHandle>,
    pub connected: bool,
    pub session_id: Option<String>,
    pub handshake_info: Option<HandshakeInfo>,
    pub pending_queries: HashMap<String, oneshot::Sender<QueryResult>>,
    pub spatial_index: SpatialIndex,
    pub delta_engine: DeltaEngine,
    pub watch_engine: WatchEngine,
    pub config: SessionConfig,
    pub clip_storage_path: Option<String>,
    pub scene_dimensions: SceneDimensions,
}

impl StageServer {
    pub fn new(state: Arc<Mutex<SessionState>>) -> Self {
        Self { state }
    }
}
```

Use `tokio::sync::Mutex` (not `std::sync::Mutex`) when the lock guard needs to be held across `.await` points. Use `std::sync::Mutex` for purely synchronous access.

```rust
// tokio Mutex — can hold across await
let mut state = self.state.lock().await;
let response = query_addon(&state.tcp_client, params).await?;  // await while locked
state.last_frame = Some(response.frame);

// std Mutex — must not hold across await, drop before awaiting
{
    let state = self.state.lock().unwrap();
    let config = state.config.clone();  // copy what you need
}   // lock dropped here
let response = query_addon(config, params).await?;  // await without lock
```

## Background Task — TCP Client

The TCP connection to the Godot addon runs as a background tokio task:

```rust
async fn tcp_client_loop(state: Arc<Mutex<SessionState>>) {
    loop {
        eprintln!("Connecting to Godot addon on :9077...");
        match TcpStream::connect("127.0.0.1:9077").await {
            Ok(stream) => {
                eprintln!("Connected to addon");
                {
                    let mut s = state.lock().await;
                    s.tcp_client = Some(TcpClientHandle::new(stream));
                }
                // Handle connection until it drops
                handle_connection(state.clone()).await;
                {
                    let mut s = state.lock().await;
                    s.tcp_client = None;
                }
                eprintln!("Addon disconnected, will retry");
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
```

In tool handlers, check `state.tcp_client` and return `not_connected` error if it's `None`:

```rust
async fn spatial_snapshot(&self, params: SpatialSnapshotParams) -> Result<String, McpError> {
    let state = self.state.lock().await;
    let client = state.tcp_client.as_ref().ok_or_else(|| {
        McpError::internal_error("Not connected to Godot addon", None)
    })?;
    // ...
}
```

## Tool Organization

Each MCP tool gets its own module in `crates/stage-server/src/mcp/`:

```
src/
├── main.rs
├── server.rs          # StageServer struct, ServerHandler impl
├── tcp.rs             # SessionState, TCP client, codec, reconnection
└── mcp/
    ├── mod.rs         # #[tool_router] impl block pulling in all tools
    ├── snapshot.rs    # spatial_snapshot implementation
    ├── delta.rs       # spatial_delta
    ├── query.rs       # spatial_query
    ├── inspect.rs     # spatial_inspect
    ├── watch.rs       # spatial_watch
    ├── config.rs      # spatial_config
    ├── action.rs      # spatial_action
    ├── scene_tree.rs  # scene_tree
    ├── clips.rs       # clips (markers, dashcam, analysis)
    ├── defaults.rs    # shared default value functions
    ├── conversions.rs # type conversion helpers
    └── responses.rs   # shared response types
```

The `#[tool_router]` can be split across multiple impl blocks. Keep each tool's logic in its own module and `pub use` what's needed.

## Common Gotchas

**Stdout is protocol-only:** Any `println!` will corrupt the MCP stdio transport. Always use `eprintln!` for debugging or configure a logger targeting stderr.

**`Clone` is required on the server struct:** The rmcp framework clones the handler for each request. Everything in your server struct must be `Clone` — use `Arc<T>` for non-Clone state.

**Tokio Mutex vs Std Mutex:** If you `.await` while holding a `std::sync::MutexGuard`, tokio will panic (or deadlock). Use `tokio::sync::Mutex` for state that's accessed across await points.

**Schemars and `Option<Vec<T>>`:** `Option<Vec<String>>` generates correct nullable array schema. `Vec<String>` generates a required array. Use `Option` for all optional list fields.

**Error messages are agent-visible:** Write `McpError` messages as if addressing the AI agent. Include the relevant values (node path, property name) so the agent can self-correct.
