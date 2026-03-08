# Design: Director Phase 1 — MVP

## Overview

Phase 1 delivers the minimum viable Director: an agent can create a scene, add
nodes, set properties, read back scene structure, and remove nodes. Everything
runs headless one-shot (no daemon, no editor plugin). The Rust MCP server is a
thin wrapper around `godot --headless --script operations.gd`.

**Tools delivered:** `scene_create`, `scene_read`, `node_add`,
`node_set_properties`, `node_remove`

**Architecture:** Rust MCP server (stdio) → spawns Godot headless subprocess →
GDScript dispatcher parses args, calls operation, prints JSON to stdout → Rust
parses JSON response.

---

## Implementation Units

### Unit 1: Workspace Scaffold

**Files:**
- `Cargo.toml` (edit — add workspace members)
- `crates/director/Cargo.toml` (new)
- `crates/director/src/lib.rs` (new)
- `crates/director/src/main.rs` (new)
- `tests/director-tests/Cargo.toml` (new)
- `tests/director-tests/src/lib.rs` (new)

**Root Cargo.toml edits:**

Add to `[workspace] members`:
```toml
members = [
    # ... existing ...
    "crates/director",
    "tests/director-tests",
]
```

Add `"tests/director-tests"` to the exclusion from default-members (like
wire-tests — requires Godot at runtime):
```toml
default-members = [
    "crates/spectator-server",
    "crates/spectator-godot",
    "crates/spectator-protocol",
    "crates/spectator-core",
    "crates/director",
]
```

**`crates/director/Cargo.toml`:**
```toml
[package]
name = "director"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "director"
path = "src/main.rs"

[dependencies]
rmcp = { version = "0.16", features = ["server", "transport-io", "macros"] }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
schemars = "1"
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow = "1"
thiserror = "2"
```

**`crates/director/src/lib.rs`:**
```rust
pub mod error;
pub mod mcp;
pub mod oneshot;
pub mod resolve;
pub mod server;
```

**`crates/director/src/main.rs`:**
```rust
use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("director=info".parse()?),
        )
        .init();

    tracing::info!("director v{}", env!("CARGO_PKG_VERSION"));

    let server = director::server::DirectorServer::new();
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    tracing::info!("MCP session ended, shutting down");
    Ok(())
}
```

**`tests/director-tests/Cargo.toml`:**
```toml
[package]
name = "director-tests"
version = "0.0.0"
edition = "2024"
publish = false

# These tests require a Godot binary (GODOT_BIN env var or `godot` in PATH).
# Run with: cargo test -p director-tests -- --include-ignored

[lib]
name = "director_tests"

[dependencies]
serde_json = "1"
anyhow = "1"
```

**`tests/director-tests/src/lib.rs`:**
```rust
mod harness;

#[cfg(test)]
mod test_scene;
#[cfg(test)]
mod test_node;
#[cfg(test)]
mod test_journey;
```

**Acceptance Criteria:**
- [ ] `cargo build -p director` succeeds
- [ ] `cargo build -p director-tests` succeeds
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo clippy --workspace` clean

---

### Unit 2: Godot Path Resolution (`resolve.rs`)

**File:** `crates/director/src/resolve.rs`

```rust
use std::path::{Path, PathBuf};

/// Resolve the Godot binary path.
///
/// Priority: `GODOT_PATH` env var → `which godot`.
pub fn resolve_godot_bin() -> Result<PathBuf, ResolveError> { ... }

/// Validate that `project_path` contains a `project.godot` file.
pub fn validate_project_path(project_path: &Path) -> Result<(), ResolveError> { ... }

/// Resolve a scene/resource path relative to the project root.
/// Returns the absolute path. Validates the parent directory exists.
pub fn resolve_scene_path(project_path: &Path, scene_path: &str) -> Result<PathBuf, ResolveError> { ... }

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("Godot binary not found. Set GODOT_PATH or add `godot` to PATH")]
    GodotNotFound,

    #[error("project_path '{0}' does not contain a project.godot file")]
    NotAProject(PathBuf),

    #[error("scene parent directory does not exist: {0}")]
    ParentMissing(PathBuf),
}
```

**Implementation Notes:**
- `resolve_godot_bin`: Check `GODOT_PATH` env var first. If unset, use
  `which::which("godot")` or shell out to `which godot`. Cache result is
  unnecessary — called once per operation.
- `validate_project_path`: Check `project_path.join("project.godot").exists()`.
- `resolve_scene_path`: Join `project_path` with `scene_path`. Verify parent
  dir exists (the file itself may not exist yet for `scene_create`).

**Acceptance Criteria:**
- [ ] `resolve_godot_bin()` returns path when `GODOT_PATH` is set
- [ ] `resolve_godot_bin()` returns path when `godot` is in PATH
- [ ] `resolve_godot_bin()` returns `GodotNotFound` when neither is available
- [ ] `validate_project_path()` succeeds for valid Godot project
- [ ] `validate_project_path()` returns `NotAProject` for non-project directory

---

### Unit 3: One-Shot Subprocess Runner (`oneshot.rs`)

**File:** `crates/director/src/oneshot.rs`

```rust
use std::path::Path;
use std::time::Duration;

/// Result of a headless Godot operation, parsed from stdout JSON.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OperationResult {
    pub success: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

impl OperationResult {
    /// Unwrap a successful result or return an error.
    pub fn into_data(self) -> Result<serde_json::Value, OperationError> { ... }
}

/// Errors from subprocess execution (not from the GDScript operation itself).
#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    #[error("Godot process failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("Godot process exited with status {status}: {stderr}")]
    ProcessFailed { status: i32, stderr: String },

    #[error("Godot process timed out after {0:?}")]
    Timeout(Duration),

    #[error("Failed to parse operation output as JSON: {source}\nstdout: {stdout}")]
    ParseFailed {
        #[source]
        source: serde_json::Error,
        stdout: String,
    },

    #[error("Operation failed: {error}")]
    OperationFailed {
        error: String,
        operation: String,
        context: serde_json::Value,
    },
}

/// Run a Director operation via headless Godot one-shot.
///
/// Spawns: `godot --headless --path <project_path> --script
/// addons/director/operations.gd <operation> '<params_json>'`
///
/// Parses the last line of stdout as JSON `OperationResult`.
pub async fn run_oneshot(
    godot_bin: &Path,
    project_path: &Path,
    operation: &str,
    params: &serde_json::Value,
) -> Result<OperationResult, OperationError> { ... }
```

**Implementation Notes:**
- Use `tokio::process::Command` for async subprocess spawning.
- Set a 30-second timeout via `tokio::time::timeout`.
- Godot prints debug info to stdout before the JSON line. Parse only the **last
  non-empty line** of stdout as JSON. All other stdout lines are debug noise.
- Stderr is captured and included in error messages but not parsed.
- The `--path` flag sets the Godot project directory. The `--script` flag
  specifies the GDScript entry point relative to the project.

**Acceptance Criteria:**
- [ ] Successful operation returns `OperationResult { success: true, data: ... }`
- [ ] Failed operation returns `OperationResult { success: false, error: ... }`
- [ ] Non-JSON stdout produces `ParseFailed` error
- [ ] Missing Godot binary produces `SpawnFailed` error
- [ ] Timeout after 30s produces `Timeout` error

---

### Unit 4: Error Types (`error.rs`)

**File:** `crates/director/src/error.rs`

```rust
use crate::oneshot::OperationError;
use crate::resolve::ResolveError;
use rmcp::model::ErrorData as McpError;

/// Convert ResolveError to McpError for use in tool handlers.
impl From<ResolveError> for McpError {
    fn from(e: ResolveError) -> Self {
        McpError::invalid_params(e.to_string(), None)
    }
}

/// Convert OperationError to McpError for use in tool handlers.
impl From<OperationError> for McpError {
    fn from(e: OperationError) -> Self {
        match &e {
            OperationError::OperationFailed { .. } => {
                McpError::internal_error(e.to_string(), None)
            }
            OperationError::SpawnFailed(_)
            | OperationError::ProcessFailed { .. }
            | OperationError::Timeout(_)
            | OperationError::ParseFailed { .. } => {
                McpError::internal_error(e.to_string(), None)
            }
        }
    }
}
```

**Implementation Notes:**
- `ResolveError` maps to `invalid_params` because it's caused by bad input
  (wrong project path, missing godot binary).
- `OperationError` maps to `internal_error` because the input was valid but
  execution failed.
- Following the error-layering pattern: library errors (`ResolveError`,
  `OperationError`) stay as typed errors; conversion to `McpError` happens at
  the handler boundary.

**Acceptance Criteria:**
- [ ] `ResolveError` converts to `McpError` with `invalid_params` code
- [ ] `OperationError` converts to `McpError` with `internal_error` code

---

### Unit 5: MCP Server + Tool Router (`server.rs`, `mcp/mod.rs`)

**File:** `crates/director/src/server.rs`

```rust
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::tool_handler;

#[derive(Clone)]
pub struct DirectorServer {
    pub tool_router: ToolRouter<Self>,
}

impl DirectorServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler]
impl ServerHandler for DirectorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "director".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
```

**File:** `crates/director/src/mcp/mod.rs`

```rust
pub mod node;
pub mod scene;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use rmcp::tool_router;

use crate::server::DirectorServer;
use crate::oneshot::{OperationResult, run_oneshot};
use crate::resolve::{resolve_godot_bin, validate_project_path};

use node::{NodeAddParams, NodeRemoveParams, NodeSetPropertiesParams};
use scene::{SceneCreateParams, SceneReadParams};

// ---------------------------------------------------------------------------
// Shared MCP helpers (Director-specific subset of Spectator's pattern)
// ---------------------------------------------------------------------------

/// Run an operation via headless Godot and return the parsed result data.
/// Handles godot resolution, project validation, subprocess, and JSON parsing.
async fn run_operation(
    project_path: &str,
    operation: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    let godot = resolve_godot_bin().map_err(McpError::from)?;
    let project = std::path::Path::new(project_path);
    validate_project_path(project).map_err(McpError::from)?;

    let result = run_oneshot(&godot, project, operation, params)
        .await
        .map_err(McpError::from)?;

    result.into_data().map_err(McpError::from)
}

fn serialize_params<T: serde::Serialize>(params: &T) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(params).map_err(|e| {
        McpError::internal_error(format!("Param serialization error: {e}"), None)
    })
}

fn serialize_response<T: serde::Serialize>(response: &T) -> Result<String, McpError> {
    serde_json::to_string(response).map_err(|e| {
        McpError::internal_error(format!("Response serialization error: {e}"), None)
    })
}

// ---------------------------------------------------------------------------
// Tool router
// ---------------------------------------------------------------------------

#[tool_router(vis = "pub")]
impl DirectorServer {
    #[tool(
        name = "scene_create",
        description = "Create a new Godot scene file (.tscn) with a specified root node type. \
            Always use this tool instead of creating .tscn files directly — the scene \
            serialization format is fragile and hand-editing will produce corrupt scenes."
    )]
    pub async fn scene_create(
        &self,
        Parameters(params): Parameters<SceneCreateParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "scene_create", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "scene_read",
        description = "Read the full node tree of a Godot scene file (.tscn) with types, \
            properties, and hierarchy. Use this to understand existing scene structure before \
            making modifications."
    )]
    pub async fn scene_read(
        &self,
        Parameters(params): Parameters<SceneReadParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "scene_read", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "node_add",
        description = "Add a node to a Godot scene file (.tscn). Optionally set initial \
            properties. Always use this tool instead of editing .tscn files directly — the scene \
            serialization format is fragile and hand-editing will produce corrupt scenes."
    )]
    pub async fn node_add(
        &self,
        Parameters(params): Parameters<NodeAddParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "node_add", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "node_set_properties",
        description = "Set properties on a node in a Godot scene file (.tscn). Handles type \
            conversion automatically (Vector2, Vector3, Color, NodePath, resource paths). \
            Always use this tool instead of editing .tscn files directly."
    )]
    pub async fn node_set_properties(
        &self,
        Parameters(params): Parameters<NodeSetPropertiesParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "node_set_properties", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "node_remove",
        description = "Remove a node (and all its children) from a Godot scene file (.tscn). \
            Always use this tool instead of editing .tscn files directly."
    )]
    pub async fn node_remove(
        &self,
        Parameters(params): Parameters<NodeRemoveParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "node_remove", &op_params).await?;
        serialize_response(&data)
    }
}
```

**Implementation Notes:**
- No `Arc<Mutex<SessionState>>` — Director Phase 1 is stateless. Each operation
  is an independent subprocess. State management comes in Phase 3 (daemon).
- No `log_activity` — Director has no addon to push activity to. May add file
  logging later.
- No `finalize_response` / budget injection — Director responses are generally
  small. Add if needed.
- `run_operation` is the Director equivalent of Spectator's `query_addon`. It
  resolves godot, validates the project, runs the subprocess, and returns
  parsed JSON.

**Acceptance Criteria:**
- [ ] `cargo build -p director` compiles with all 5 tool methods
- [ ] Tool descriptions include anti-direct-edit guidance
- [ ] Each handler delegates to `run_operation` (thin handlers)

---

### Unit 6: Scene Tool Params (`mcp/scene.rs`)

**File:** `crates/director/src/mcp/scene.rs`

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `scene_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SceneCreateParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Path to the scene file relative to the project root (e.g., "scenes/player.tscn").
    pub scene_path: String,

    /// The Godot class name for the root node (e.g., "Node2D", "Node3D", "Control").
    pub root_type: String,
}

/// Parameters for `scene_read`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SceneReadParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Path to the scene file relative to the project root (e.g., "scenes/player.tscn").
    pub scene_path: String,

    /// Maximum tree depth to include (default: unlimited).
    #[serde(default)]
    pub depth: Option<u32>,

    /// Whether to include node properties in the output (default: true).
    #[serde(default = "default_true")]
    pub properties: bool,
}

fn default_true() -> bool {
    true
}
```

**Acceptance Criteria:**
- [ ] `SceneCreateParams` has `project_path`, `scene_path`, `root_type` (all required)
- [ ] `SceneReadParams` has `project_path`, `scene_path` (required), `depth` and `properties` (optional)
- [ ] `properties` defaults to `true`
- [ ] All params derive `JsonSchema` for MCP schema generation

---

### Unit 7: Node Tool Params (`mcp/node.rs`)

**File:** `crates/director/src/mcp/node.rs`

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `node_add`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeAddParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the parent node within the scene tree (e.g., "." for root, "Player" for
    /// a child named Player). Default: root node (".").
    #[serde(default = "default_root")]
    pub parent_path: String,

    /// The Godot class name for the new node (e.g., "Sprite2D", "CollisionShape2D").
    pub node_type: String,

    /// Name for the new node.
    pub node_name: String,

    /// Optional initial properties to set on the node after creation.
    /// Keys are property names, values are JSON representations of the property values.
    /// Type conversion is handled automatically (e.g., {"x": 100, "y": 200} for Vector2).
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Parameters for `node_set_properties`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetPropertiesParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree (e.g., "Player/Sprite2D").
    pub node_path: String,

    /// Properties to set. Keys are property names, values are JSON representations.
    /// Type conversion is automatic: Vector2 from {"x":1,"y":2}, Color from "#ff0000"
    /// or {"r":1,"g":0,"b":0}, NodePath from string, resources from "res://" paths.
    pub properties: serde_json::Map<String, serde_json::Value>,
}

/// Parameters for `node_remove`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeRemoveParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the node to remove within the scene tree.
    /// All children of this node are also removed.
    pub node_path: String,
}

fn default_root() -> String {
    ".".to_string()
}
```

**Acceptance Criteria:**
- [ ] `NodeAddParams` has required: `project_path`, `scene_path`, `node_type`, `node_name`; optional: `parent_path` (default "."), `properties`
- [ ] `NodeSetPropertiesParams` has required: `project_path`, `scene_path`, `node_path`, `properties`
- [ ] `NodeRemoveParams` has required: `project_path`, `scene_path`, `node_path`
- [ ] Property value descriptions explain type conversion in JsonSchema docstrings

---

### Unit 8: GDScript Operations Dispatcher (`addons/director/operations.gd`)

**File:** `addons/director/operations.gd`

```gdscript
extends SceneTree

## Director headless operations dispatcher.
## Called via: godot --headless --path <project> --script addons/director/operations.gd <op> '<json>'

func _init():
    var args = _parse_args()
    if args.error != "":
        _print_error(args.error, "parse_args", {})
        quit(1)
        return

    var result = {}
    match args.operation:
        "scene_create":
            result = SceneOps.op_scene_create(args.params)
        "scene_read":
            result = SceneOps.op_scene_read(args.params)
        "node_add":
            result = NodeOps.op_node_add(args.params)
        "node_set_properties":
            result = NodeOps.op_node_set_properties(args.params)
        "node_remove":
            result = NodeOps.op_node_remove(args.params)
        _:
            result = {"success": false, "error": "Unknown operation: " + args.operation, "operation": args.operation, "context": {}}

    print(JSON.stringify(result))
    quit(0)


func _parse_args() -> Dictionary:
    var cmdline = OS.get_cmdline_user_args()
    if cmdline.size() < 2:
        return {"error": "Usage: operations.gd <operation> '<json_params>'", "operation": "", "params": {}}

    var operation = cmdline[0]
    var json_str = cmdline[1]
    var json = JSON.new()
    var err = json.parse(json_str)
    if err != OK:
        return {"error": "Invalid JSON: " + json.get_error_message(), "operation": operation, "params": {}}

    return {"error": "", "operation": operation, "params": json.get_data()}


func _print_error(message: String, operation: String, context: Dictionary):
    var result = {
        "success": false,
        "error": message,
        "operation": operation,
        "context": context,
    }
    print(JSON.stringify(result))
```

**Implementation Notes:**
- Uses `OS.get_cmdline_user_args()` which returns args after `--` in Godot's
  command line. When using `--script`, user args are those after the script
  path.
- `SceneOps` and `NodeOps` are preloaded scripts — see Units 9 and 10.
- `quit(0)` after printing ensures clean exit (non-zero for parse errors only).
- Only the last `print()` call matters — the Rust side parses the last non-empty
  line of stdout.

**Acceptance Criteria:**
- [ ] Dispatches to correct operation function based on first arg
- [ ] Parses JSON params from second arg
- [ ] Returns structured error JSON for unknown operations
- [ ] Returns structured error JSON for invalid JSON input
- [ ] Exits with `quit(0)` on success, `quit(1)` on parse error

---

### Unit 9: Scene Operations GDScript (`addons/director/ops/scene_ops.gd`)

**File:** `addons/director/ops/scene_ops.gd`

```gdscript
class_name SceneOps


static func op_scene_create(params: Dictionary) -> Dictionary:
    ## Create a new scene with the specified root node type.
    ##
    ## Params: scene_path (String), root_type (String)
    ## Returns: { success, data: { path, root_type } }

    var scene_path: String = params.get("scene_path", "")
    var root_type: String = params.get("root_type", "")

    if scene_path == "":
        return _error("scene_path is required", "scene_create", params)
    if root_type == "":
        return _error("root_type is required", "scene_create", params)

    # Validate the class exists
    if not ClassDB.class_exists(root_type):
        return _error("Unknown node type: " + root_type, "scene_create", {"scene_path": scene_path, "root_type": root_type})

    # Ensure the class is a Node subclass
    if not ClassDB.is_parent_class(root_type, "Node"):
        return _error(root_type + " is not a Node subclass", "scene_create", {"scene_path": scene_path, "root_type": root_type})

    # Create the root node
    var root = ClassDB.instantiate(root_type)
    root.name = _name_from_path(scene_path)

    # Pack into a scene
    var packed = PackedScene.new()
    var err = packed.pack(root)
    root.queue_free()
    if err != OK:
        return _error("Failed to pack scene: " + str(err), "scene_create", {"scene_path": scene_path})

    # Ensure parent directory exists
    var full_path = "res://" + scene_path
    var dir_path = full_path.get_base_dir()
    if not DirAccess.dir_exists_absolute(dir_path):
        DirAccess.make_dir_recursive_absolute(dir_path)

    # Save
    err = ResourceSaver.save(packed, full_path)
    if err != OK:
        return _error("Failed to save scene: " + str(err), "scene_create", {"scene_path": scene_path})

    return {"success": true, "data": {"path": scene_path, "root_type": root_type}}


static func op_scene_read(params: Dictionary) -> Dictionary:
    ## Read the full node tree of a scene file.
    ##
    ## Params: scene_path (String), depth (int, optional), properties (bool, default true)
    ## Returns: { success, data: { root: NodeData } }

    var scene_path: String = params.get("scene_path", "")
    if scene_path == "":
        return _error("scene_path is required", "scene_read", params)

    var full_path = "res://" + scene_path
    if not ResourceLoader.exists(full_path):
        return _error("Scene not found: " + scene_path, "scene_read", {"scene_path": scene_path})

    var packed: PackedScene = load(full_path)
    if packed == null:
        return _error("Failed to load scene: " + scene_path, "scene_read", {"scene_path": scene_path})

    var root = packed.instantiate()
    if root == null:
        return _error("Failed to instantiate scene: " + scene_path, "scene_read", {"scene_path": scene_path})

    var max_depth: int = params.get("depth", -1)
    var include_props: bool = params.get("properties", true)

    var node_data = _read_node(root, 0, max_depth, include_props)
    root.queue_free()

    return {"success": true, "data": {"root": node_data}}


static func _read_node(node: Node, current_depth: int, max_depth: int, include_props: bool) -> Dictionary:
    var data: Dictionary = {
        "name": node.name,
        "type": node.get_class(),
    }

    if include_props:
        data["properties"] = _get_serializable_properties(node)

    if max_depth < 0 or current_depth < max_depth:
        var children: Array = []
        for child in node.get_children():
            children.append(_read_node(child, current_depth + 1, max_depth, include_props))
        if children.size() > 0:
            data["children"] = children

    return data


static func _get_serializable_properties(node: Node) -> Dictionary:
    ## Extract non-default, user-relevant properties from a node.
    var props: Dictionary = {}
    var defaults = ClassDB.instantiate(node.get_class())

    for prop_info in node.get_property_list():
        var name: String = prop_info["name"]
        # Skip internal/meta properties
        if name.begins_with("_") or name == "script" or prop_info["usage"] & PROPERTY_USAGE_CATEGORY:
            continue
        if prop_info["usage"] & PROPERTY_USAGE_EDITOR == 0:
            continue

        var value = node.get(name)
        var default_value = defaults.get(name) if defaults else null

        # Only include non-default values
        if defaults and value == default_value:
            continue

        props[name] = _serialize_value(value)

    if defaults:
        defaults.queue_free()
    return props


static func _serialize_value(value) -> Variant:
    ## Convert a Godot value to a JSON-safe representation.
    if value is Vector2:
        return {"x": value.x, "y": value.y}
    elif value is Vector3:
        return {"x": value.x, "y": value.y, "z": value.z}
    elif value is Color:
        return {"r": value.r, "g": value.g, "b": value.b, "a": value.a}
    elif value is NodePath:
        return str(value)
    elif value is Resource:
        return value.resource_path if value.resource_path != "" else str(value)
    elif value is Rect2:
        return {"position": {"x": value.position.x, "y": value.position.y}, "size": {"x": value.size.x, "y": value.size.y}}
    elif value is Transform2D:
        return {"origin": {"x": value.origin.x, "y": value.origin.y}, "x": {"x": value.x.x, "y": value.x.y}, "y": {"x": value.y.x, "y": value.y.y}}
    elif value is Basis:
        return {"x": {"x": value.x.x, "y": value.x.y, "z": value.x.z}, "y": {"x": value.y.x, "y": value.y.y, "z": value.y.z}, "z": {"x": value.z.x, "y": value.z.y, "z": value.z.z}}
    elif value is Transform3D:
        return {"basis": _serialize_value(value.basis), "origin": {"x": value.origin.x, "y": value.origin.y, "z": value.origin.z}}
    elif value is Array:
        var arr = []
        for item in value:
            arr.append(_serialize_value(item))
        return arr
    elif value is Dictionary:
        var dict = {}
        for key in value:
            dict[str(key)] = _serialize_value(value[key])
        return dict
    else:
        return value


static func _name_from_path(scene_path: String) -> String:
    ## Extract a node name from a scene path: "scenes/player.tscn" → "Player"
    var file_name = scene_path.get_file().get_basename()
    return file_name.capitalize().replace(" ", "")


static func _error(message: String, operation: String, context: Dictionary) -> Dictionary:
    return {"success": false, "error": message, "operation": operation, "context": context}
```

**Implementation Notes:**
- `ClassDB.instantiate()` is used instead of `.new()` so we can create nodes by
  string class name.
- `_get_serializable_properties` creates a default instance to compare against,
  so only non-default values are included in output. This reduces noise.
- `_serialize_value` converts Godot types to JSON-safe equivalents. This is the
  reverse of `convert_value` (Unit 10).
- `queue_free()` is called on instantiated nodes to prevent leaks in headless
  mode. Since we run in `_init()` (before the tree is running), `queue_free`
  schedules cleanup but won't actually run. Instead, the process exits.
  Alternative: just don't worry about it — the process is about to exit anyway.
  Use `free()` directly if `queue_free` doesn't work in `_init` context.

**Acceptance Criteria:**
- [ ] `op_scene_create` creates a valid `.tscn` file loadable by Godot
- [ ] `op_scene_create` returns error for invalid `root_type`
- [ ] `op_scene_create` creates parent directories if needed
- [ ] `op_scene_read` returns full node tree with name, type, children
- [ ] `op_scene_read` includes only non-default properties when `properties: true`
- [ ] `op_scene_read` respects `depth` limit
- [ ] `op_scene_read` returns error for non-existent scene
- [ ] Vector2, Vector3, Color serialize to `{x, y}`, `{x, y, z}`, `{r, g, b, a}`

---

### Unit 10: Node Operations GDScript (`addons/director/ops/node_ops.gd`)

**File:** `addons/director/ops/node_ops.gd`

```gdscript
class_name NodeOps


static func op_node_add(params: Dictionary) -> Dictionary:
    ## Add a node to an existing scene.
    ##
    ## Params: scene_path, parent_path (default "."), node_type, node_name, properties (optional)
    ## Returns: { success, data: { node_path, type } }

    var scene_path: String = params.get("scene_path", "")
    var parent_path: String = params.get("parent_path", ".")
    var node_type: String = params.get("node_type", "")
    var node_name: String = params.get("node_name", "")
    var properties = params.get("properties", null)

    if scene_path == "":
        return _error("scene_path is required", "node_add", params)
    if node_type == "":
        return _error("node_type is required", "node_add", params)
    if node_name == "":
        return _error("node_name is required", "node_add", params)

    var full_path = "res://" + scene_path
    if not ResourceLoader.exists(full_path):
        return _error("Scene not found: " + scene_path, "node_add", {"scene_path": scene_path})

    if not ClassDB.class_exists(node_type):
        return _error("Unknown node type: " + node_type, "node_add", {"node_type": node_type})
    if not ClassDB.is_parent_class(node_type, "Node"):
        return _error(node_type + " is not a Node subclass", "node_add", {"node_type": node_type})

    # Load and instantiate the scene
    var packed: PackedScene = load(full_path)
    var root = packed.instantiate()

    # Find the parent node
    var parent: Node
    if parent_path == "." or parent_path == "":
        parent = root
    else:
        parent = root.get_node_or_null(parent_path)
    if parent == null:
        root.free()
        return _error("Parent node not found: " + parent_path, "node_add", {"scene_path": scene_path, "parent_path": parent_path})

    # Create and add the new node
    var new_node = ClassDB.instantiate(node_type)
    new_node.name = node_name
    parent.add_child(new_node)
    new_node.owner = root  # Required for PackedScene serialization

    # Set properties if provided
    if properties is Dictionary:
        var prop_result = _set_properties_on_node(new_node, properties)
        if not prop_result.success:
            root.free()
            return prop_result

    # Re-pack and save
    var save_result = _repack_and_save(root, full_path)
    if not save_result.success:
        return save_result

    var result_path = str(root.get_path_to(new_node))
    root.free()

    return {"success": true, "data": {"node_path": result_path, "type": node_type}}


static func op_node_set_properties(params: Dictionary) -> Dictionary:
    ## Set properties on an existing node in a scene.
    ##
    ## Params: scene_path, node_path, properties (Dictionary)
    ## Returns: { success, data: { node_path, properties_set: [] } }

    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var properties = params.get("properties", {})

    if scene_path == "":
        return _error("scene_path is required", "node_set_properties", params)
    if node_path == "":
        return _error("node_path is required", "node_set_properties", params)
    if not properties is Dictionary or properties.is_empty():
        return _error("properties must be a non-empty dictionary", "node_set_properties", params)

    var full_path = "res://" + scene_path
    if not ResourceLoader.exists(full_path):
        return _error("Scene not found: " + scene_path, "node_set_properties", {"scene_path": scene_path})

    var packed: PackedScene = load(full_path)
    var root = packed.instantiate()

    # Find the target node
    var target: Node
    if node_path == "." or node_path == "":
        target = root
    else:
        target = root.get_node_or_null(node_path)
    if target == null:
        root.free()
        return _error("Node not found: " + node_path, "node_set_properties", {"scene_path": scene_path, "node_path": node_path})

    # Set properties with type conversion
    var set_result = _set_properties_on_node(target, properties)
    if not set_result.success:
        root.free()
        return set_result

    # Re-pack and save
    var save_result = _repack_and_save(root, full_path)
    root.free()
    if not save_result.success:
        return save_result

    return {"success": true, "data": {"node_path": node_path, "properties_set": set_result.properties_set}}


static func op_node_remove(params: Dictionary) -> Dictionary:
    ## Remove a node (and all children) from a scene.
    ##
    ## Params: scene_path, node_path
    ## Returns: { success, data: { removed: node_path, children_removed: int } }

    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")

    if scene_path == "":
        return _error("scene_path is required", "node_remove", params)
    if node_path == "" or node_path == ".":
        return _error("Cannot remove root node", "node_remove", {"scene_path": scene_path})

    var full_path = "res://" + scene_path
    if not ResourceLoader.exists(full_path):
        return _error("Scene not found: " + scene_path, "node_remove", {"scene_path": scene_path})

    var packed: PackedScene = load(full_path)
    var root = packed.instantiate()

    var target = root.get_node_or_null(node_path)
    if target == null:
        root.free()
        return _error("Node not found: " + node_path, "node_remove", {"scene_path": scene_path, "node_path": node_path})

    var children_count = _count_descendants(target)
    target.get_parent().remove_child(target)
    target.free()

    # Re-pack and save
    var save_result = _repack_and_save(root, full_path)
    root.free()
    if not save_result.success:
        return save_result

    return {"success": true, "data": {"removed": node_path, "children_removed": children_count}}


# ---------------------------------------------------------------------------
# Type conversion system
# ---------------------------------------------------------------------------

static func convert_value(value, expected_type: int):
    ## Convert a JSON value to the expected Godot type.
    ## Called by _set_properties_on_node after querying get_property_list().
    match expected_type:
        TYPE_BOOL:
            return bool(value)
        TYPE_INT:
            return int(value)
        TYPE_FLOAT:
            return float(value)
        TYPE_STRING:
            return str(value)
        TYPE_VECTOR2:
            if value is Dictionary:
                return Vector2(value.get("x", 0), value.get("y", 0))
            return value
        TYPE_VECTOR2I:
            if value is Dictionary:
                return Vector2i(int(value.get("x", 0)), int(value.get("y", 0)))
            return value
        TYPE_VECTOR3:
            if value is Dictionary:
                return Vector3(value.get("x", 0), value.get("y", 0), value.get("z", 0))
            return value
        TYPE_VECTOR3I:
            if value is Dictionary:
                return Vector3i(int(value.get("x", 0)), int(value.get("y", 0)), int(value.get("z", 0)))
            return value
        TYPE_COLOR:
            if value is String:
                return Color.html(value)
            if value is Dictionary:
                return Color(value.get("r", 0), value.get("g", 0), value.get("b", 0), value.get("a", 1.0))
            return value
        TYPE_NODE_PATH:
            return NodePath(str(value))
        TYPE_OBJECT:
            if value is String and str(value).begins_with("res://"):
                return load(str(value))
            return value
        TYPE_RECT2:
            if value is Dictionary:
                var pos = value.get("position", {"x": 0, "y": 0})
                var sz = value.get("size", {"x": 0, "y": 0})
                return Rect2(pos.get("x", 0), pos.get("y", 0), sz.get("x", 0), sz.get("y", 0))
            return value
        _:
            return value


static func _set_properties_on_node(node: Node, properties: Dictionary) -> Dictionary:
    ## Set multiple properties on a node with automatic type conversion.
    ## Returns { success: true, properties_set: [...] } or { success: false, error: ... }
    var properties_set: Array = []
    var prop_list = node.get_property_list()
    var type_map: Dictionary = {}
    for prop_info in prop_list:
        type_map[prop_info["name"]] = prop_info["type"]

    for prop_name in properties:
        var value = properties[prop_name]

        if not type_map.has(prop_name):
            return {"success": false, "error": "Unknown property: " + prop_name + " on " + node.get_class(), "operation": "node_set_properties", "context": {"node": str(node.name), "property": prop_name}}

        var expected_type = type_map[prop_name]
        var converted = convert_value(value, expected_type)
        node.set(prop_name, converted)
        properties_set.append(prop_name)

    return {"success": true, "properties_set": properties_set}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

static func _repack_and_save(root: Node, full_path: String) -> Dictionary:
    ## Re-pack a modified node tree and save it to disk.
    var packed = PackedScene.new()
    # Set ownership for all descendants so they get included in the packed scene
    _set_owner_recursive(root, root)
    var err = packed.pack(root)
    if err != OK:
        return {"success": false, "error": "Failed to pack scene: " + str(err), "operation": "save", "context": {"path": full_path}}
    err = ResourceSaver.save(packed, full_path)
    if err != OK:
        return {"success": false, "error": "Failed to save scene: " + str(err), "operation": "save", "context": {"path": full_path}}
    return {"success": true}


static func _set_owner_recursive(node: Node, owner: Node):
    ## Set owner on all descendants (required for PackedScene serialization).
    for child in node.get_children():
        child.owner = owner
        _set_owner_recursive(child, owner)


static func _count_descendants(node: Node) -> int:
    var count = 0
    for child in node.get_children():
        count += 1 + _count_descendants(child)
    return count


static func _error(message: String, operation: String, context: Dictionary) -> Dictionary:
    return {"success": false, "error": message, "operation": operation, "context": context}
```

**Implementation Notes:**
- **Owner chain is critical.** When packing a scene, only nodes whose `owner`
  is the root get serialized. `_set_owner_recursive` must be called before
  `packed.pack(root)`. The root node itself must NOT have its owner set (it's
  the owner).
- **Type conversion via `get_property_list()`.** We build a `type_map` from the
  node's property list, then `convert_value` handles the JSON→Godot conversion.
  This is the GDScript mirror of `VariantTarget` on the Rust side.
- **Load → instantiate → modify → repack → save** is the standard pattern for
  modifying `.tscn` files via Godot's API.
- **Error on unknown property** rather than silently ignoring — follows the
  contract rule that schema fields must be forwarded or return an explicit error.

**Acceptance Criteria:**
- [ ] `op_node_add` adds a node to an existing scene and saves
- [ ] `op_node_add` with `properties` sets initial values
- [ ] `op_node_add` returns error for non-existent parent
- [ ] `op_node_add` returns error for invalid node type
- [ ] `op_node_set_properties` sets properties with type conversion
- [ ] `op_node_set_properties` handles Vector2, Vector3, Color, NodePath, resource paths
- [ ] `op_node_set_properties` returns error for unknown property name
- [ ] `op_node_remove` removes a node and all children
- [ ] `op_node_remove` prevents removing root node
- [ ] `_set_owner_recursive` ensures all nodes serialize into the packed scene
- [ ] `convert_value` handles all TYPE_* variants listed

---

### Unit 11: GDScript Plugin Stub (`addons/director/plugin.cfg`, `addons/director/plugin.gd`)

**File:** `addons/director/plugin.cfg`
```ini
[plugin]

name="Director"
description="AI agent scene and resource manipulation for Godot"
author="Theatre"
version="0.1.0"
script="plugin.gd"
```

**File:** `addons/director/plugin.gd`
```gdscript
@tool
extends EditorPlugin

func _enter_tree():
    pass

func _exit_tree():
    pass
```

**Implementation Notes:**
- This is a stub. The editor plugin TCP listener comes in Phase 7.
- Having `plugin.cfg` present means users can enable the addon in Project
  Settings, but it does nothing yet. This is intentional — the addon directory
  must exist for `--script addons/director/operations.gd` to work.
- The `@tool` annotation is required for EditorPlugin scripts.

**Acceptance Criteria:**
- [ ] `plugin.cfg` is valid and Godot can parse it
- [ ] `plugin.gd` extends EditorPlugin with no errors
- [ ] Addon directory structure allows `--script addons/director/operations.gd` to work

---

### Unit 12: Test Harness (`tests/director-tests/src/harness.rs`)

**File:** `tests/director-tests/src/harness.rs`

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

/// A Director operation runner for E2E tests.
///
/// Spawns `godot --headless --path <project> --script addons/director/operations.gd
/// <op> '<json>'` and parses the JSON result from stdout.
pub struct DirectorFixture {
    godot_bin: String,
    project_dir: PathBuf,
}

/// Parsed operation result from GDScript stdout.
#[derive(Debug, serde::Deserialize)]
pub struct OperationResult {
    pub success: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

impl OperationResult {
    pub fn unwrap_data(self) -> serde_json::Value {
        if !self.success {
            panic!(
                "Expected success, got error: {}",
                self.error.unwrap_or_else(|| "unknown".into())
            );
        }
        self.data
    }

    pub fn unwrap_err(self) -> String {
        if self.success {
            panic!("Expected error, got success: {:?}", self.data);
        }
        self.error.unwrap_or_else(|| "unknown error".into())
    }
}

impl DirectorFixture {
    pub fn new() -> Self {
        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
        Self {
            godot_bin,
            project_dir: Self::project_dir(),
        }
    }

    /// Run a Director operation and return the parsed result.
    pub fn run(&self, operation: &str, params: serde_json::Value) -> anyhow::Result<OperationResult> {
        let output = Command::new(&self.godot_bin)
            .args([
                "--headless",
                "--path",
                &self.project_dir.to_string_lossy(),
                "--script",
                "addons/director/operations.gd",
                "--",
                operation,
                &params.to_string(),
            ])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to launch Godot ({}): {e}", self.godot_bin))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse the last non-empty line of stdout as JSON
        let json_line = stdout
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("No output from Godot.\nstderr: {stderr}")
            })?;

        serde_json::from_str(json_line).map_err(|e| {
            anyhow::anyhow!("Failed to parse JSON: {e}\nline: {json_line}\nfull stdout: {stdout}\nstderr: {stderr}")
        })
    }

    /// Create a temporary scene file path that won't conflict between tests.
    pub fn temp_scene_path(name: &str) -> String {
        format!("tmp/test_{name}.tscn")
    }

    fn project_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../godot-project")
            .canonicalize()
            .expect("tests/godot-project dir must exist")
    }
}

/// Assert two f64 values are approximately equal (within 0.01).
pub fn assert_approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected ~{expected}, got {actual}"
    );
}
```

**Implementation Notes:**
- Uses `--` separator between script path and user args. This is how Godot
  passes args to `OS.get_cmdline_user_args()`.
- Parses last non-empty line — Godot may print debug info before our JSON.
- `temp_scene_path` generates paths under `tmp/` to keep test artifacts
  isolated. The `tmp/` dir in the test project should be `.gitignore`d.
- Same `assert_approx` helper as wire-tests.

**Acceptance Criteria:**
- [ ] `DirectorFixture::run` successfully spawns Godot and parses JSON output
- [ ] `OperationResult::unwrap_data` panics on error results
- [ ] `OperationResult::unwrap_err` panics on success results
- [ ] Test scene paths are isolated under `tmp/`

---

### Unit 13: E2E Tests

**File:** `tests/director-tests/src/test_scene.rs`

```rust
use crate::harness::{DirectorFixture, OperationResult};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_create_then_read_round_trips() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("create_read");

    // Create
    let result = f.run("scene_create", json!({
        "scene_path": scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();
    assert_eq!(result["root_type"], "Node2D");
    assert_eq!(result["path"], scene);

    // Read back
    let result = f.run("scene_read", json!({
        "scene_path": scene
    })).unwrap().unwrap_data();
    assert_eq!(result["root"]["type"], "Node2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_create_invalid_type_returns_error() {
    let f = DirectorFixture::new();
    let err = f.run("scene_create", json!({
        "scene_path": DirectorFixture::temp_scene_path("invalid_type"),
        "root_type": "NotARealClass"
    })).unwrap().unwrap_err();
    assert!(err.contains("Unknown node type"));
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_read_nonexistent_returns_error() {
    let f = DirectorFixture::new();
    let err = f.run("scene_read", json!({
        "scene_path": "nonexistent/missing.tscn"
    })).unwrap().unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_read_with_depth_limit() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("depth_limit");

    // Create scene with nested nodes
    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"})).unwrap();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Child"})).unwrap();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "Child", "node_type": "Sprite2D", "node_name": "Grandchild"})).unwrap();

    // Read with depth=1 — should include root + direct children, not grandchildren
    let result = f.run("scene_read", json!({"scene_path": scene, "depth": 1})).unwrap().unwrap_data();
    let root = &result["root"];
    assert!(root["children"].as_array().is_some());
    let child = &root["children"][0];
    assert!(child.get("children").is_none() || child["children"].as_array().unwrap().is_empty());
}
```

**File:** `tests/director-tests/src/test_node.rs`

```rust
use crate::harness::{DirectorFixture, assert_approx};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn node_add_to_root() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("node_add_root");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"})).unwrap();

    let result = f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "Sprite2D",
        "node_name": "MySprite"
    })).unwrap().unwrap_data();

    assert_eq!(result["type"], "Sprite2D");
    assert_eq!(result["node_path"], "MySprite");
}

#[test]
#[ignore = "requires Godot binary"]
fn node_add_with_properties() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("node_add_props");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"})).unwrap();
    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "Sprite2D",
        "node_name": "S",
        "properties": {"position": {"x": 100, "y": 200}, "visible": false}
    })).unwrap().unwrap_data();

    // Verify via scene_read
    let data = f.run("scene_read", json!({"scene_path": scene})).unwrap().unwrap_data();
    let sprite = &data["root"]["children"][0];
    assert_eq!(sprite["name"], "S");
    assert_approx(sprite["properties"]["position"]["x"].as_f64().unwrap(), 100.0);
    assert_approx(sprite["properties"]["position"]["y"].as_f64().unwrap(), 200.0);
    assert_eq!(sprite["properties"]["visible"], false);
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_properties_vector2() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("set_props_v2");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"})).unwrap();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Sprite2D", "node_name": "S"})).unwrap();

    let result = f.run("node_set_properties", json!({
        "scene_path": scene,
        "node_path": "S",
        "properties": {"position": {"x": 50, "y": 75}}
    })).unwrap().unwrap_data();

    assert!(result["properties_set"].as_array().unwrap().contains(&json!("position")));

    // Verify
    let data = f.run("scene_read", json!({"scene_path": scene})).unwrap().unwrap_data();
    let pos = &data["root"]["children"][0]["properties"]["position"];
    assert_approx(pos["x"].as_f64().unwrap(), 50.0);
    assert_approx(pos["y"].as_f64().unwrap(), 75.0);
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_properties_unknown_property_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("set_props_unknown");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"})).unwrap();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Sprite2D", "node_name": "S"})).unwrap();

    let err = f.run("node_set_properties", json!({
        "scene_path": scene,
        "node_path": "S",
        "properties": {"nonexistent_property": 42}
    })).unwrap().unwrap_err();

    assert!(err.contains("Unknown property"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_remove_with_children() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("node_remove");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"})).unwrap();
    f.run("node_add", json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Parent"})).unwrap();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "Parent", "node_type": "Sprite2D", "node_name": "Child"})).unwrap();

    let result = f.run("node_remove", json!({
        "scene_path": scene,
        "node_path": "Parent"
    })).unwrap().unwrap_data();

    assert_eq!(result["removed"], "Parent");
    assert_eq!(result["children_removed"], 1);

    // Verify parent is gone
    let data = f.run("scene_read", json!({"scene_path": scene})).unwrap().unwrap_data();
    assert!(data["root"].get("children").is_none() || data["root"]["children"].as_array().unwrap().is_empty());
}

#[test]
#[ignore = "requires Godot binary"]
fn node_remove_root_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("remove_root");

    f.run("scene_create", json!({"scene_path": scene, "root_type": "Node2D"})).unwrap();

    let err = f.run("node_remove", json!({
        "scene_path": scene,
        "node_path": "."
    })).unwrap().unwrap_err();

    assert!(err.contains("root"));
}
```

**File:** `tests/director-tests/src/test_journey.rs`

```rust
use crate::harness::{DirectorFixture, assert_approx};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_create_scene_add_nodes_set_properties_read_back() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("journey_full");

    // 1. Create scene
    f.run("scene_create", json!({
        "scene_path": scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    // 2. Add a player node
    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "CharacterBody2D",
        "node_name": "Player"
    })).unwrap().unwrap_data();

    // 3. Add a sprite to the player
    f.run("node_add", json!({
        "scene_path": scene,
        "parent_path": "Player",
        "node_type": "Sprite2D",
        "node_name": "Sprite"
    })).unwrap().unwrap_data();

    // 4. Add a collision shape to the player
    f.run("node_add", json!({
        "scene_path": scene,
        "parent_path": "Player",
        "node_type": "CollisionShape2D",
        "node_name": "Collision"
    })).unwrap().unwrap_data();

    // 5. Set position on the player
    f.run("node_set_properties", json!({
        "scene_path": scene,
        "node_path": "Player",
        "properties": {"position": {"x": 200, "y": 300}}
    })).unwrap().unwrap_data();

    // 6. Read back and verify full tree
    let data = f.run("scene_read", json!({
        "scene_path": scene
    })).unwrap().unwrap_data();

    let root = &data["root"];
    assert_eq!(root["type"], "Node2D");

    let player = &root["children"][0];
    assert_eq!(player["name"], "Player");
    assert_eq!(player["type"], "CharacterBody2D");
    assert_approx(player["properties"]["position"]["x"].as_f64().unwrap(), 200.0);
    assert_approx(player["properties"]["position"]["y"].as_f64().unwrap(), 300.0);

    let children = player["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["name"], "Sprite");
    assert_eq!(children[0]["type"], "Sprite2D");
    assert_eq!(children[1]["name"], "Collision");
    assert_eq!(children[1]["type"], "CollisionShape2D");

    // 7. Remove the sprite
    f.run("node_remove", json!({
        "scene_path": scene,
        "node_path": "Player/Sprite"
    })).unwrap().unwrap_data();

    // 8. Verify removal
    let data = f.run("scene_read", json!({
        "scene_path": scene
    })).unwrap().unwrap_data();
    let player = &data["root"]["children"][0];
    let children = player["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "Collision");
}
```

**Acceptance Criteria:**
- [ ] All tests pass with `cargo test -p director-tests -- --include-ignored`
- [ ] Tests cover: scene create/read round-trip, invalid types, nonexistent scenes
- [ ] Tests cover: node add (root, nested), node add with properties
- [ ] Tests cover: node set properties (Vector2, unknown property error)
- [ ] Tests cover: node remove (with children, root rejection)
- [ ] Tests cover: full journey (create → add → set → read → remove → verify)

---

### Unit 14: Test Project Setup

**Files:**
- `tests/godot-project/addons/director/` (symlink or copy of `addons/director/`)
- `tests/godot-project/tmp/` (directory, `.gitignore`d)
- `tests/godot-project/.gitignore` (edit — add `tmp/`)

**Implementation Notes:**
- The test project at `tests/godot-project/` already has an `addons/` directory
  (with `spectator/`). Director's addon must also be accessible.
- **Symlink approach**: Create a symlink `tests/godot-project/addons/director`
  → `../../../../addons/director`. This ensures tests always use the current
  addon code.
- Create `tests/godot-project/tmp/` directory for test scene artifacts. Add
  `tmp/` to the project's `.gitignore`.

**Acceptance Criteria:**
- [ ] `tests/godot-project/addons/director/operations.gd` is accessible
- [ ] `tests/godot-project/tmp/` exists and is gitignored
- [ ] `godot --headless --path tests/godot-project --script addons/director/operations.gd -- scene_read '{}'` runs without crashing (returns an error JSON)

---

## Implementation Order

1. **Unit 11**: GDScript plugin stub (`plugin.cfg`, `plugin.gd`) — needed before anything
2. **Unit 8**: GDScript operations dispatcher (`operations.gd`)
3. **Unit 9**: Scene operations GDScript (`ops/scene_ops.gd`)
4. **Unit 10**: Node operations GDScript (`ops/node_ops.gd`)
5. **Unit 14**: Test project setup (symlink, tmp dir)
6. **Unit 1**: Workspace scaffold (Cargo.toml files, lib.rs, main.rs)
7. **Unit 2**: Godot path resolution (`resolve.rs`)
8. **Unit 3**: One-shot subprocess runner (`oneshot.rs`)
9. **Unit 4**: Error types (`error.rs`)
10. **Unit 6**: Scene tool params (`mcp/scene.rs`)
11. **Unit 7**: Node tool params (`mcp/node.rs`)
12. **Unit 5**: MCP server + tool router (`server.rs`, `mcp/mod.rs`)
13. **Unit 12**: Test harness (`harness.rs`)
14. **Unit 13**: E2E tests

**Rationale:** GDScript first because it can be tested manually with `godot
--headless` before any Rust is written. Rust scaffold next, then resolution →
subprocess → error → params → router (dependency chain). Tests last because
they need everything.

---

## Testing

### E2E Tests: `tests/director-tests/src/`

All tests use `#[ignore = "requires Godot binary"]` and run with:
```bash
cargo test -p director-tests -- --include-ignored
```

**Test structure:**
- `test_scene.rs` — scene create/read operations
- `test_node.rs` — node add/set/remove operations
- `test_journey.rs` — multi-step end-to-end workflows

**Test isolation:** Each test creates scenes under `tmp/test_<name>.tscn` to
avoid conflicts. Tests are independent — each creates its own scene from
scratch.

### Manual GDScript Testing

Before writing any Rust, validate GDScript operations manually:
```bash
cd tests/godot-project
godot --headless --script addons/director/operations.gd -- scene_create '{"scene_path":"tmp/manual_test.tscn","root_type":"Node2D"}'
godot --headless --script addons/director/operations.gd -- scene_read '{"scene_path":"tmp/manual_test.tscn"}'
```

### Rust Unit Tests

`resolve.rs` should have inline `#[cfg(test)] mod tests` for:
- Godot resolution with/without env var
- Project path validation
- Scene path resolution

---

## Verification Checklist

```bash
# Build
cargo build --workspace
cargo clippy --workspace
cargo fmt --check

# Unit tests (no Godot needed)
cargo test -p director

# E2E tests (requires Godot)
cargo test -p director-tests -- --include-ignored

# Manual GDScript smoke test
cd tests/godot-project
godot --headless --script addons/director/operations.gd -- scene_create '{"scene_path":"tmp/verify.tscn","root_type":"Node2D"}'
godot --headless --script addons/director/operations.gd -- scene_read '{"scene_path":"tmp/verify.tscn"}'
godot --headless --script addons/director/operations.gd -- node_add '{"scene_path":"tmp/verify.tscn","node_type":"Sprite2D","node_name":"Test"}'
godot --headless --script addons/director/operations.gd -- scene_read '{"scene_path":"tmp/verify.tscn"}'
```
