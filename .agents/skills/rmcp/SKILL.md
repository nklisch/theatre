---
name: rmcp
description: Working with the rmcp Rust MCP SDK in the stage-server crate. Use when writing or modifying MCP tool definitions, server initialization, or tool call handling.
---

# rmcp — Rust MCP Server SDK

This skill covers the `rmcp` crate used in `crates/stage-server`. The MCP server exposes Stage's tools to AI agents via stdio transport. For the current catalog, read the `#[tool]` methods in `crates/stage-server/src/mcp/mod.rs` rather than trusting a count here.

## Cargo.toml

```toml
[dependencies]
rmcp = { version = "0.16", features = ["server", "transport-io", "macros"] }
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
    // Build shared state first, then the server. `StageServer::new` attaches
    // per-tool output schemas to the tool router (see `router_with_schemas`).
    let state = Arc::new(Mutex::new(tcp::SessionState {
        config: base_config,
        project_dashcam_config: config::load_toml_dashcam(&project_dir),
        project_dir: project_dir.clone(),
        ..Default::default()
    }));
    let server = StageServer::new(state.clone());

    // Keep reconnect ownership on the server so project_select can replace it.
    server.start_connection(port).await?;

    // Start MCP server on stdio — this blocks until client disconnects
    server.serve(stdio()).await?.waiting().await?;

    Ok(())
}
```

**Key point:** Start the server-owned connection before serving MCP. Do not spawn
an independent reconnect loop for a persistent MCP server: `project_select` must
be able to stop and join that task before replacing session state. Ordinary MCP
calls share the operation lock in `ServerHandler::call_tool`; selection holds it
exclusively through teardown and its response.

**stderr for logging:** stdout is MCP protocol only. Use `eprintln!` or a logger configured to write to stderr.

## Defining Tools — The `#[tool_router]` Pattern

Tools live on a struct that derives `Clone` (required for shared state pattern):

```rust
#[derive(Clone)]
pub struct StageServer {
    pub state: Arc<Mutex<SessionState>>,
    pub tool_router: ToolRouter<Self>,
}

#[tool_router(vis = "pub")]
impl StageServer {
    #[tool(description = "Get a spatial snapshot of the current scene from a perspective")]
    pub async fn spatial_snapshot(
        &self,
        Parameters(params): Parameters<SpatialSnapshotParams>,
    ) -> Result<rmcp::model::CallToolResult, McpError> {
        let result = handle_snapshot(params, &self.state).await;
        result.and_then(stage_protocol::mcp_helpers::structured_json)
    }
}
```

`#[tool_router]` on the impl block auto-generates tool listing and routing into a `ToolRouter` held on the struct. `#[tool(description = "...")]` on each method registers it as an MCP tool. The description is what the AI model sees — write it from the agent's perspective ("Get", "Returns", "Query"). `Parameters<T>` provides the typed input boundary; the shared `handle_*` JSON handler stays `Result<String, McpError>` for CLI reuse, and the tool method converts with `structured_json`.

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

MCP tool methods return `Result<rmcp::model::CallToolResult, McpError>`. Shared JSON handlers return `Result<String, McpError>` so the CLI can reuse them; `stage_protocol::mcp_helpers::structured_json` converts that JSON into matching text and structured content. Image-producing handlers construct their `CallToolResult` with image blocks directly; do not pass those results through the JSON-string helper.

```rust
use rmcp::model::ErrorData as McpError;

// Success — handler returns JSON text; the MCP wrapper converts it
let result = handle_snapshot(params, &self.state).await; // Result<String, McpError>
result.and_then(stage_protocol::mcp_helpers::structured_json)

// Structured error with code
Err(McpError::invalid_params("Node 'enemies/scout_99' not found", None))

// Internal error
Err(McpError::internal_error("TCP connection lost", None))
```

**Standard error constructors on `McpError`:**
- `McpError::invalid_params(message, data)` — bad agent input
- `McpError::internal_error(message, data)` — server/addon side failure

These two constructors are all the codebase uses; do not invent custom error codes.

**Distinguish agent errors from server errors:**
- Agent's fault (bad node path, invalid params) → `invalid_params` → agent can fix and retry
- Our fault (TCP drop, serialization fail) → `internal_error` → agent should report

## Implementing `ServerHandler`

`#[tool_router]` generates the tool router, not Theatre's `ServerHandler` implementation. The server explicitly implements `call_tool`, `list_tools`, `get_tool`, and `get_info`. The metadata portion is:

```rust
impl ServerHandler for StageServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "stage-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
    }
}
```

`StageServer::router_with_schemas()` attaches the declared output schemas to the router. `call_tool` dispatches through it and appends a pending-feedback notice; `list_tools` and `get_tool` expose its entries. Follow `crates/stage-server/src/server.rs` for the complete implementation.

## Shared State

```rust
#[derive(Clone)]
pub struct StageServer {
    pub state: Arc<Mutex<SessionState>>,  // tokio::sync::Mutex for async
    pub tool_router: ToolRouter<Self>,    // built by router_with_schemas()
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
        Self { state, tool_router: Self::router_with_schemas() }
    }
}
```

Session state uses `tokio::sync::Mutex`. Clone needed configuration under a short lock and release it before querying the addon. `query_addon` takes `&Arc<Mutex<SessionState>>`, acquires its own lock to register/send the query, and releases that lock before waiting for the response. Do not call it while holding the session lock.

```rust
let config = self.state.lock().await.config.clone();
// The temporary lock guard is dropped at the end of the previous statement.
let response = query_addon(&self.state, method, params).await?;
```

Use `std::sync::Mutex` only for synchronous ownership that does not require holding a guard across `.await`.

## Background Task — TCP Client

The TCP connection runs as a background Tokio task owned by `ProjectConnection`
(`crates/stage-server/src/project.rs`). Start it through `StageServer::start_connection`.
The lower-level loop below is suitable for transport tests, not an independent
persistent-MCP task:

```rust
async fn tcp_client_loop(state: Arc<Mutex<SessionState>>, port: u16) {
    loop {
        tracing::info!("Connecting to Godot addon on 127.0.0.1:{}...", port);
        match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
            Ok(stream) => {
                tracing::info!("Connected to addon");
                // Handshake, query serving, and tcp_writer state updates all
                // live inside handle_connection until the stream drops.
                if let Err(e) = handle_connection(stream, state.clone()).await {
                    tracing::warn!("Connection error: {e}");
                }
                tracing::info!("Addon disconnected, will retry in 2s");
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
```

In tool handlers, check `state.tcp_writer` and return a not-connected internal error if it's `None`:

```rust
async fn spatial_snapshot(&self, params: SpatialSnapshotParams) -> Result<CallToolResult, McpError> {
    let state = self.state.lock().await;
    let client = state.tcp_writer.as_ref().ok_or_else(|| {
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
├── server.rs          # StageServer struct, ServerHandler impl, output schemas
├── tcp.rs             # SessionState, TCP client, codec, reconnection
├── activity.rs        # best-effort activity log events pushed to the addon
├── cli.rs             # one-shot CLI invocation mode
├── clip_analysis.rs   # saved-clip SQLite reads and analysis (ClipSession)
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
    ├── viewport.rs    # viewport (mixed text + image content)
    ├── runtime_status.rs      # runtime_status
    ├── runtime_diagnostics.rs # runtime_diagnostics
    ├── feedback.rs    # feedback (project-local, no engine query)
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
