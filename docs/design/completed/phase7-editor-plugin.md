# Design: Phase 7 — Editor Plugin Backend

## Overview

When the Godot editor is open with the Director plugin active, operations
route through the live editor API instead of headless. This provides:
- Immediate viewport feedback (changes appear live)
- Dirty-scene safety (reads see unsaved changes, modifications don't overwrite them)
- Filesystem sync (new/modified files appear in the FileSystem dock instantly)

Phase 7 adds three components:
1. **Rust TCP client** (`editor.rs`) — connects to the editor plugin on `:6551`
2. **Backend selection update** (`backend.rs`) — editor > daemon > one-shot priority
3. **GDScript editor plugin** (`plugin.gd` + `editor_ops.gd`) — TCP listener +
   EditorInterface API dispatch

### Backend Routing Rules

| Scenario | Backend |
|---|---|
| Editor plugin connected | Editor (all operations) |
| Editor not connected | Daemon > one-shot (unchanged) |

Within the editor plugin, operations dispatch based on scene state:

| Operation targets... | Dispatch path |
|---|---|
| Currently active scene (live in editor) | Editor-specific live tree code |
| Open but non-active scene | Headless ops/ + `reload_scene_from_path()` |
| Non-open scene or resource-only op | Headless ops/ + `filesystem.scan()` |

**Known limitation:** Modifications to open-but-non-active scenes go through
disk (headless ops/) then reload. If those scenes have unsaved changes, the
unsaved changes are lost. The most common workflow — agent working on the
currently active scene — is fully safe. Future work can address non-active
scenes by switching tabs programmatically.

### Port Resolution

Editor plugin port is resolved in order:
1. `DIRECTOR_EDITOR_PORT` env var (highest priority)
2. `project.godot` setting: `[director] connection/editor_port=<port>`
3. Default: `6551`

Both the Rust client and GDScript plugin use the same resolution order.

---

## Implementation Units

### Unit 1: EditorHandle (Rust TCP Client)

**File**: `crates/director/src/editor.rs`

```rust
use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::oneshot::OperationResult;

const DEFAULT_PORT: u16 = 6551;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors specific to the editor plugin TCP client.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("editor plugin not reachable on port {0}")]
    NotReachable(u16),

    #[error("editor plugin TCP I/O error: {0}")]
    IoError(#[source] std::io::Error),

    #[error("editor plugin response parse error: {source}\nraw: {raw}")]
    ParseFailed {
        #[source]
        source: serde_json::Error,
        raw: String,
    },

    #[error("editor plugin operation timed out")]
    Timeout,
}

/// TCP client handle for a running Director EditorPlugin.
///
/// Unlike DaemonHandle, this does not manage a process — the editor
/// is already running. EditorHandle only manages the TCP connection.
pub struct EditorHandle {
    stream: TcpStream,
    port: u16,
}

impl EditorHandle {
    /// Attempt to connect to the editor plugin on the given port.
    ///
    /// Returns `Err(EditorError::NotReachable)` if the plugin is not
    /// listening (editor closed or plugin not enabled). The connect
    /// attempt times out after CONNECT_TIMEOUT (2s).
    pub async fn connect(port: u16) -> Result<Self, EditorError> {
        let addr = format!("127.0.0.1:{port}");
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr))
            .await
            .map_err(|_| EditorError::NotReachable(port))?
            .map_err(|_| EditorError::NotReachable(port))?;
        Ok(EditorHandle { stream, port })
    }

    /// Send an operation and return the result.
    ///
    /// Wire format: length-prefixed JSON (4-byte BE u32 + JSON payload),
    /// identical to the daemon protocol.
    pub async fn send_operation(
        &mut self,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, EditorError> {
        let request = serde_json::json!({
            "operation": operation,
            "params": params,
        });

        tokio::time::timeout(OPERATION_TIMEOUT, async {
            write_message(&mut self.stream, &request).await?;
            let response = read_message(&mut self.stream).await?;
            serde_json::from_value(response).map_err(|source| EditorError::ParseFailed {
                source,
                raw: String::new(),
            })
        })
        .await
        .map_err(|_| EditorError::Timeout)?
    }

    /// Check if the TCP connection is still alive (non-blocking peek).
    pub fn is_alive(&self) -> bool {
        // A zero-byte peek succeeds if the socket is open.
        // WouldBlock means alive but no data; Err means dead.
        match self.stream.try_read(&mut [0u8; 0]) {
            Ok(_) => true,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Resolve the editor plugin port.
///
/// Priority: DIRECTOR_EDITOR_PORT env var > project.godot setting > default 6551.
pub fn resolve_editor_port(project_path: &Path) -> u16 {
    // 1. Env var
    if let Ok(val) = std::env::var("DIRECTOR_EDITOR_PORT") {
        if let Ok(port) = val.parse::<u16>() {
            return port;
        }
    }

    // 2. project.godot
    let godot_file = project_path.join("project.godot");
    if let Ok(contents) = std::fs::read_to_string(&godot_file) {
        if let Some(port) = parse_editor_port_from_project(&contents) {
            return port;
        }
    }

    // 3. Default
    DEFAULT_PORT
}

/// Parse the editor port from project.godot content.
///
/// Looks for `connection/editor_port=<number>` under the `[director]` section.
fn parse_editor_port_from_project(contents: &str) -> Option<u16> {
    let mut in_director_section = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_director_section = trimmed == "[director]";
            continue;
        }
        if in_director_section {
            if let Some(val) = trimmed.strip_prefix("connection/editor_port=") {
                return val.trim().trim_matches('"').parse().ok();
            }
        }
    }
    None
}

// -- Wire format (identical to daemon.rs) -----------------------------------

async fn write_message(stream: &mut TcpStream, value: &serde_json::Value) -> Result<(), EditorError> {
    let json = serde_json::to_vec(value).map_err(|source| EditorError::ParseFailed {
        source,
        raw: String::new(),
    })?;
    let len = (json.len() as u32).to_be_bytes();
    stream.write_all(&len).await.map_err(EditorError::IoError)?;
    stream.write_all(&json).await.map_err(EditorError::IoError)?;
    stream.flush().await.map_err(EditorError::IoError)?;
    Ok(())
}

async fn read_message(stream: &mut TcpStream) -> Result<serde_json::Value, EditorError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(EditorError::IoError)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await.map_err(EditorError::IoError)?;
    let raw = String::from_utf8_lossy(&buf).into_owned();
    serde_json::from_slice(&buf).map_err(|source| EditorError::ParseFailed { source, raw })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_default_port() {
        unsafe { std::env::remove_var("DIRECTOR_EDITOR_PORT") };
        let port = resolve_editor_port(Path::new("/nonexistent"));
        assert_eq!(port, 6551);
    }

    #[test]
    fn resolve_env_var_port() {
        unsafe { std::env::set_var("DIRECTOR_EDITOR_PORT", "7777") };
        let port = resolve_editor_port(Path::new("/nonexistent"));
        assert_eq!(port, 7777);
        unsafe { std::env::remove_var("DIRECTOR_EDITOR_PORT") };
    }

    #[test]
    fn parse_project_godot_port() {
        let contents = "\
[application]\nconfig/name=\"Test\"\n\n[director]\nconnection/editor_port=6600\n";
        assert_eq!(parse_editor_port_from_project(contents), Some(6600));
    }

    #[test]
    fn parse_project_godot_no_section() {
        let contents = "[application]\nconfig/name=\"Test\"\n";
        assert_eq!(parse_editor_port_from_project(contents), None);
    }

    #[test]
    fn parse_project_godot_wrong_section() {
        let contents = "[stage]\nconnection/editor_port=6600\n";
        assert_eq!(parse_editor_port_from_project(contents), None);
    }
}
```

**Implementation Notes**:
- Wire format (length-prefixed JSON) is identical to `daemon.rs`. The
  `write_message`/`read_message` functions are duplicated rather than
  extracted to a shared module, matching the existing daemon pattern.
  If a third backend is ever added, extract to a shared `wire.rs`.
- `is_alive()` uses a zero-byte `try_read` instead of `try_wait()` (no
  process to wait on — the editor is not managed by Director).
- `CONNECT_TIMEOUT` is 2s (short) because the editor plugin should respond
  immediately if running. This keeps the fallback to daemon fast.

**Acceptance Criteria**:
- [ ] `resolve_editor_port` returns env var value when set
- [ ] `resolve_editor_port` parses `project.godot` `[director]` section
- [ ] `resolve_editor_port` returns 6551 as default
- [ ] `EditorHandle::connect` returns `NotReachable` when nothing listens
- [ ] `EditorHandle::send_operation` sends length-prefixed JSON and parses response
- [ ] All unit tests pass

---

### Unit 2: Backend Selection Update

**File**: `crates/director/src/backend.rs`

```rust
use tokio::sync::Mutex;

use crate::daemon::{DaemonHandle, resolve_daemon_port};
use crate::editor::{EditorHandle, EditorError, resolve_editor_port};
use crate::oneshot::{self, OperationError, OperationResult};

/// Backend selection: editor plugin → daemon → one-shot fallback.
pub struct Backend {
    editor: Mutex<Option<EditorHandle>>,
    daemon: Mutex<Option<DaemonHandle>>,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            editor: Mutex::new(None),
            daemon: Mutex::new(None),
        }
    }

    pub async fn run_operation(
        &self,
        godot_bin: &Path,
        project_path: &Path,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, OperationError> {
        // 1. Try editor plugin
        match self.try_editor(project_path, operation, params).await {
            Ok(result) => return Ok(result),
            Err(EditorError::NotReachable(_)) => {
                // Editor not running — fall through to daemon
            }
            Err(e) => {
                tracing::warn!("editor plugin failed ({e}), falling through to daemon");
            }
        }

        // 2. Try daemon → one-shot (existing logic, unchanged)
        self.try_daemon_then_oneshot(godot_bin, project_path, operation, params).await
    }

    /// Attempt to run an operation via the editor plugin.
    async fn try_editor(
        &self,
        project_path: &Path,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, EditorError> {
        let port = resolve_editor_port(project_path);
        let mut guard = self.editor.lock().await;

        // Use cached connection if alive.
        if let Some(ref mut handle) = *guard {
            if handle.is_alive() {
                match handle.send_operation(operation, params).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        tracing::warn!("editor send failed ({e}), reconnecting");
                        *guard = None;
                    }
                }
            } else {
                *guard = None;
            }
        }

        // Try fresh connection.
        let mut handle = EditorHandle::connect(port).await?;
        let result = handle.send_operation(operation, params).await?;
        *guard = Some(handle);
        Ok(result)
    }

    /// Existing daemon → one-shot logic (extracted from current run_operation).
    async fn try_daemon_then_oneshot(
        &self,
        godot_bin: &Path,
        project_path: &Path,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, OperationError> {
        // ... existing daemon + one-shot fallback code, unchanged ...
    }

    pub async fn shutdown(&self) {
        // Disconnect editor (drop the handle — no process to kill).
        {
            let mut guard = self.editor.lock().await;
            *guard = None;
        }
        // Shut down daemon (existing code).
        {
            let mut guard = self.daemon.lock().await;
            if let Some(handle) = guard.take() {
                if let Err(e) = handle.shutdown().await {
                    tracing::warn!("daemon shutdown error: {e}");
                }
            }
        }
    }
}
```

**Implementation Notes**:
- The existing daemon + one-shot code in `run_operation` moves into
  `try_daemon_then_oneshot` unchanged. The new `run_operation` calls
  `try_editor` first, then falls through.
- Editor connection is cached in `Mutex<Option<EditorHandle>>`, same
  pattern as the daemon handle.
- `EditorError::NotReachable` is the "editor not running" signal — it
  triggers silent fallthrough (no warning log). Other editor errors
  (I/O, parse, timeout) log a warning before falling through.
- `shutdown()` drops the editor handle (TCP disconnect). No process
  kill needed since the editor is user-managed.

**Acceptance Criteria**:
- [ ] When editor plugin is listening, operations route through it
- [ ] When editor is not listening, operations fall through to daemon/one-shot
- [ ] Editor connection is cached between operations
- [ ] Stale editor connection triggers reconnect attempt
- [ ] Failed reconnect falls through to daemon
- [ ] `shutdown()` disconnects editor and shuts down daemon

---

### Unit 3: Error Type Registration

**File**: `crates/director/src/error.rs`

```rust
use crate::editor::EditorError;

/// Convert EditorError to McpError for use in tool handlers.
impl From<EditorError> for McpError {
    fn from(e: EditorError) -> Self {
        match e {
            EditorError::NotReachable(_) => {
                McpError::internal_error(e.to_string(), None)
            }
            EditorError::IoError(_)
            | EditorError::ParseFailed { .. }
            | EditorError::Timeout => {
                McpError::internal_error(e.to_string(), None)
            }
        }
    }
}
```

Also add `From<EditorError> for OperationError` in `editor.rs`:

```rust
impl From<EditorError> for OperationError {
    fn from(e: EditorError) -> Self {
        OperationError::ProcessFailed {
            status: -1,
            stderr: e.to_string(),
        }
    }
}
```

**Acceptance Criteria**:
- [ ] `EditorError` converts to `McpError` via `From`
- [ ] `EditorError` converts to `OperationError` via `From`

---

### Unit 4: Module Registration

**File**: `crates/director/src/lib.rs`

```rust
pub mod backend;
pub mod daemon;
pub mod editor;   // <-- new
pub mod error;
pub mod mcp;
pub mod oneshot;
pub mod resolve;
pub mod server;
```

**Acceptance Criteria**:
- [ ] `cargo build -p director` compiles with the new module

---

### Unit 5: EditorPlugin TCP Listener (plugin.gd)

**File**: `addons/director/plugin.gd`

```gdscript
@tool
extends EditorPlugin

const EditorOps = preload("res://addons/director/editor_ops.gd")

const DEFAULT_PORT := 6551
const SETTING_PATH := "director/connection/editor_port"

var _server: TCPServer
var _client: StreamPeerTCP
var _read_buf: PackedByteArray = PackedByteArray()
var _port: int


func _enter_tree() -> void:
    _register_settings()
    _port = _resolve_port()

    _server = TCPServer.new()
    var err = _server.listen(_port)
    if err != OK:
        printerr("[Director] Failed to listen on port %d (error %d)" % [_port, err])
        return

    print("[Director] Editor plugin listening on port %d" % _port)


func _exit_tree() -> void:
    if _client != null and _client.get_status() == StreamPeerTCP.STATUS_CONNECTED:
        _client.disconnect_from_host()
    _client = null
    if _server != null:
        _server.stop()
    _server = null
    print("[Director] Editor plugin stopped")


func _process(_delta: float) -> void:
    if _server == null:
        return
    _accept_client()
    _poll_client()


func _accept_client() -> void:
    if not _server.is_connection_available():
        return
    # Disconnect existing client before accepting new one.
    if _client != null and _client.get_status() == StreamPeerTCP.STATUS_CONNECTED:
        _client.disconnect_from_host()
    _client = _server.take_connection()
    _read_buf.clear()


func _poll_client() -> void:
    if _client == null:
        return
    _client.poll()

    var status = _client.get_status()
    if status == StreamPeerTCP.STATUS_NONE or status == StreamPeerTCP.STATUS_ERROR:
        _client = null
        _read_buf.clear()
        return
    if status != StreamPeerTCP.STATUS_CONNECTED:
        return

    # Drain available bytes.
    var available = _client.get_available_bytes()
    if available > 0:
        var res = _client.get_data(available)
        if res[0] == OK:
            _read_buf.append_array(res[1] as PackedByteArray)

    # Try to decode one message per frame.
    var msg = _try_decode_message()
    if msg.is_empty():
        return

    var operation: String = msg.get("operation", "")
    var params: Dictionary = msg.get("params", {})

    if operation == "ping":
        _send_message({"success": true, "data": {"status": "ok", "backend": "editor"}, "operation": "ping"})
        return

    var result = EditorOps.dispatch(operation, params)
    _send_message(result)


func _try_decode_message() -> Dictionary:
    # Identical to daemon.gd — length-prefixed JSON decoding.
    if _read_buf.size() < 4:
        return {}
    var msg_len: int = (_read_buf[0] << 24) | (_read_buf[1] << 16) | (_read_buf[2] << 8) | _read_buf[3]
    if msg_len == 0:
        _read_buf = _read_buf.slice(4)
        return {}
    if _read_buf.size() < 4 + msg_len:
        return {}
    var msg_bytes: PackedByteArray = _read_buf.slice(4, 4 + msg_len)
    _read_buf = _read_buf.slice(4 + msg_len)
    var json_str = msg_bytes.get_string_from_utf8()
    var json = JSON.new()
    if json.parse(json_str) != OK:
        return {}
    var data = json.get_data()
    if typeof(data) != TYPE_DICTIONARY:
        return {}
    return data


func _send_message(data: Dictionary) -> void:
    # Identical to daemon.gd — length-prefixed JSON encoding.
    var json_str = JSON.stringify(data)
    var json_bytes: PackedByteArray = json_str.to_utf8_buffer()
    var msg_len = json_bytes.size()
    var len_bytes = PackedByteArray([
        (msg_len >> 24) & 0xFF,
        (msg_len >> 16) & 0xFF,
        (msg_len >> 8) & 0xFF,
        msg_len & 0xFF,
    ])
    _client.put_data(len_bytes)
    _client.put_data(json_bytes)


func _resolve_port() -> int:
    # 1. Env var
    if OS.has_environment("DIRECTOR_EDITOR_PORT"):
        var val = int(OS.get_environment("DIRECTOR_EDITOR_PORT"))
        if val > 0:
            return val
    # 2. Project setting
    if ProjectSettings.has_setting(SETTING_PATH):
        var val = int(ProjectSettings.get_setting(SETTING_PATH))
        if val > 0:
            return val
    # 3. Default
    return DEFAULT_PORT


func _register_settings() -> void:
    _add_setting(SETTING_PATH, TYPE_INT, DEFAULT_PORT,
        PROPERTY_HINT_RANGE, "1024,65535")


func _add_setting(path: String, type: int, default_value: Variant,
        hint: int = PROPERTY_HINT_NONE, hint_string: String = "") -> void:
    if not ProjectSettings.has_setting(path):
        ProjectSettings.set_setting(path, default_value)
    ProjectSettings.set_initial_value(path, default_value)
    ProjectSettings.add_property_info({
        "name": path,
        "type": type,
        "hint": hint,
        "hint_string": hint_string,
    })
```

**Implementation Notes**:
- TCP message framing is identical to `daemon.gd`. Both use 4-byte BE u32
  length prefix + JSON payload.
- No idle timeout — the plugin stays alive as long as the editor is open.
- No quit operation — the editor manages the plugin lifecycle via
  `_enter_tree`/`_exit_tree`.
- `ping` response includes `"backend": "editor"` so the Rust client can
  verify it reached the editor (vs the daemon, which returns `"status": "ok"`).
- Port resolution follows the same priority as the Rust side: env var >
  project setting > default.
- `_process` is used (not `_physics_process`) since this runs in the editor,
  not in a game loop.
- Settings are registered following the Stage plugin pattern
  (`_register_settings` + `_add_setting`).

**Acceptance Criteria**:
- [ ] Plugin listens on configured port when enabled in editor
- [ ] Plugin accepts TCP connections and receives length-prefixed JSON
- [ ] Plugin dispatches operations to `EditorOps.dispatch()`
- [ ] Plugin returns length-prefixed JSON responses
- [ ] Plugin reads port from env var, project settings, or default
- [ ] Plugin stops listener on `_exit_tree`

---

### Unit 6: Editor Operations Dispatcher (editor_ops.gd)

**File**: `addons/director/editor_ops.gd`

```gdscript
class_name EditorOps

## Editor-context operation dispatcher.
##
## Routes operations based on whether the target scene is the currently
## active scene in the editor:
##   - Active scene → live tree manipulation via EditorInterface
##   - Non-active/non-open scene → delegate to ops/ + reload/scan
##   - Resource-only operations → delegate to ops/ + scan

const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")
const TileMapOps = preload("res://addons/director/ops/tilemap_ops.gd")
const GridMapOps = preload("res://addons/director/ops/gridmap_ops.gd")
const AnimationOps = preload("res://addons/director/ops/animation_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")

## Scene-targeting operations that can use live tree manipulation.
const SCENE_OPS := [
    "scene_read", "node_add", "node_set_properties", "node_remove",
    "node_reparent", "scene_add_instance",
    "tilemap_set_cells", "tilemap_get_cells", "tilemap_clear",
    "gridmap_set_cells", "gridmap_get_cells", "gridmap_clear",
]


static func dispatch(operation: String, params: Dictionary) -> Dictionary:
    ## Main entry point. Called by plugin.gd for every incoming operation.
    var scene_path: String = params.get("scene_path", "")

    # For scene-targeting operations, check if the scene is the active tab.
    if scene_path != "" and operation in SCENE_OPS:
        var active_root := _get_active_scene_root(scene_path)
        if active_root != null:
            return _dispatch_live(operation, params, active_root)

    # All other cases: delegate to headless ops + editor sync.
    var result := _dispatch_headless(operation, params)
    _post_operation_sync(operation, params, result)
    return result


# ---------------------------------------------------------------------------
# Active scene detection
# ---------------------------------------------------------------------------

static func _get_active_scene_root(scene_path: String) -> Node:
    ## Returns the scene root if scene_path is the currently active editor scene.
    ## Returns null otherwise.
    var full_path := "res://" + scene_path
    var root := EditorInterface.get_edited_scene_root()
    if root != null and root.scene_file_path == full_path:
        return root
    return null


# ---------------------------------------------------------------------------
# Live tree operations (active scene)
# ---------------------------------------------------------------------------

static func _dispatch_live(operation: String, params: Dictionary, scene_root: Node) -> Dictionary:
    match operation:
        "scene_read":
            return _live_scene_read(params, scene_root)
        "node_add":
            return _live_node_add(params, scene_root)
        "node_set_properties":
            return _live_node_set_properties(params, scene_root)
        "node_remove":
            return _live_node_remove(params, scene_root)
        "node_reparent":
            return _live_node_reparent(params, scene_root)
        "scene_add_instance":
            return _live_scene_add_instance(params, scene_root)
        "tilemap_set_cells":
            return _live_tilemap_set_cells(params, scene_root)
        "tilemap_get_cells":
            return _live_tilemap_get_cells(params, scene_root)
        "tilemap_clear":
            return _live_tilemap_clear(params, scene_root)
        "gridmap_set_cells":
            return _live_gridmap_set_cells(params, scene_root)
        "gridmap_get_cells":
            return _live_gridmap_get_cells(params, scene_root)
        "gridmap_clear":
            return _live_gridmap_clear(params, scene_root)
        _:
            return OpsUtil._error("Unknown live operation: " + operation, operation, params)


static func _live_scene_read(params: Dictionary, scene_root: Node) -> Dictionary:
    ## Read the live scene tree (sees unsaved changes).
    ## Reuses SceneOps serialization helpers on the live root.
    var depth: int = params.get("depth", -1)
    var include_props: bool = params.get("properties", true)
    var root_data := SceneOps._serialize_node(scene_root, depth, 0, include_props)
    return {"success": true, "data": {"root": root_data}}


static func _live_node_add(params: Dictionary, scene_root: Node) -> Dictionary:
    ## Add a node to the live scene tree.
    var parent_path: String = params.get("parent_path", "")
    var node_type: String = params.get("node_type", "")
    var node_name: String = params.get("node_name", "")
    var properties: Dictionary = params.get("properties", {})

    if node_type == "":
        return OpsUtil._error("node_type is required", "node_add", params)
    if node_name == "":
        return OpsUtil._error("node_name is required", "node_add", params)
    if not ClassDB.class_exists(node_type):
        return OpsUtil._error("Unknown node type: " + node_type, "node_add", params)
    if not ClassDB.is_parent_class(node_type, "Node"):
        return OpsUtil._error(node_type + " is not a Node subclass", "node_add", params)

    var parent: Node = _resolve_node(scene_root, parent_path)
    if parent == null:
        return OpsUtil._error("Parent node not found: " + parent_path, "node_add", params)

    var node: Node = ClassDB.instantiate(node_type)
    node.name = node_name
    parent.add_child(node)
    node.owner = scene_root

    # Set properties if provided.
    if not properties.is_empty():
        NodeOps._apply_properties(node, properties)

    var node_path := str(scene_root.get_path_to(node))
    return {"success": true, "data": {"node_path": node_path, "type": node_type}}


static func _live_node_set_properties(params: Dictionary, scene_root: Node) -> Dictionary:
    ## Set properties on a node in the live scene tree.
    var node_path: String = params.get("node_path", "")
    var properties: Dictionary = params.get("properties", {})

    if node_path == "":
        return OpsUtil._error("node_path is required", "node_set_properties", params)

    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "node_set_properties", params)

    var set_props: Array = NodeOps._apply_properties(node, properties)
    return {"success": true, "data": {"node_path": node_path, "properties_set": set_props}}


static func _live_node_remove(params: Dictionary, scene_root: Node) -> Dictionary:
    ## Remove a node from the live scene tree.
    var node_path: String = params.get("node_path", "")

    if node_path == "":
        return OpsUtil._error("node_path is required", "node_remove", params)

    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "node_remove", params)
    if node == scene_root:
        return OpsUtil._error("Cannot remove scene root", "node_remove", params)

    var children_count := node.get_child_count()
    node.get_parent().remove_child(node)
    node.queue_free()

    return {"success": true, "data": {"removed": node_path, "children_removed": children_count}}


static func _live_node_reparent(params: Dictionary, scene_root: Node) -> Dictionary:
    ## Reparent a node within the live scene tree.
    var node_path: String = params.get("node_path", "")
    var new_parent_path: String = params.get("new_parent_path", "")

    if node_path == "":
        return OpsUtil._error("node_path is required", "node_reparent", params)
    if new_parent_path == "":
        return OpsUtil._error("new_parent_path is required", "node_reparent", params)

    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "node_reparent", params)

    var new_parent: Node = _resolve_node(scene_root, new_parent_path)
    if new_parent == null:
        return OpsUtil._error("New parent not found: " + new_parent_path, "node_reparent", params)

    var old_path := str(scene_root.get_path_to(node))
    node.reparent(new_parent)
    var new_path := str(scene_root.get_path_to(node))

    return {"success": true, "data": {"old_path": old_path, "new_path": new_path}}


static func _live_scene_add_instance(params: Dictionary, scene_root: Node) -> Dictionary:
    ## Add a scene instance to the live scene tree.
    var instance_scene: String = params.get("instance_scene", "")
    var parent_path: String = params.get("parent_path", "")
    var node_name: String = params.get("node_name", "")

    if instance_scene == "":
        return OpsUtil._error("instance_scene is required", "scene_add_instance", params)

    var full_scene_path := "res://" + instance_scene
    if not ResourceLoader.exists(full_scene_path):
        return OpsUtil._error("Scene not found: " + instance_scene, "scene_add_instance", params)

    var packed: PackedScene = load(full_scene_path)
    if packed == null:
        return OpsUtil._error("Failed to load scene: " + instance_scene, "scene_add_instance", params)

    var parent: Node = _resolve_node(scene_root, parent_path)
    if parent == null:
        return OpsUtil._error("Parent node not found: " + parent_path, "scene_add_instance", params)

    var instance: Node = packed.instantiate()
    if node_name != "":
        instance.name = node_name
    parent.add_child(instance)
    instance.owner = scene_root

    # Set owner recursively for all children of the instance.
    _set_owner_recursive(instance, scene_root)

    var result_path := str(scene_root.get_path_to(instance))
    return {"success": true, "data": {"node_path": result_path, "instance_scene": instance_scene}}


static func _live_tilemap_set_cells(params: Dictionary, scene_root: Node) -> Dictionary:
    var node_path: String = params.get("node_path", "")
    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "tilemap_set_cells", params)
    # Delegate to TileMapOps with the live node.
    return TileMapOps._set_cells_on_node(node, params)


static func _live_tilemap_get_cells(params: Dictionary, scene_root: Node) -> Dictionary:
    var node_path: String = params.get("node_path", "")
    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "tilemap_get_cells", params)
    return TileMapOps._get_cells_from_node(node, params)


static func _live_tilemap_clear(params: Dictionary, scene_root: Node) -> Dictionary:
    var node_path: String = params.get("node_path", "")
    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "tilemap_clear", params)
    return TileMapOps._clear_node(node, params)


static func _live_gridmap_set_cells(params: Dictionary, scene_root: Node) -> Dictionary:
    var node_path: String = params.get("node_path", "")
    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "gridmap_set_cells", params)
    return GridMapOps._set_cells_on_node(node, params)


static func _live_gridmap_get_cells(params: Dictionary, scene_root: Node) -> Dictionary:
    var node_path: String = params.get("node_path", "")
    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "gridmap_get_cells", params)
    return GridMapOps._get_cells_from_node(node, params)


static func _live_gridmap_clear(params: Dictionary, scene_root: Node) -> Dictionary:
    var node_path: String = params.get("node_path", "")
    var node: Node = _resolve_node(scene_root, node_path)
    if node == null:
        return OpsUtil._error("Node not found: " + node_path, "gridmap_clear", params)
    return GridMapOps._clear_node(node, params)


# ---------------------------------------------------------------------------
# Headless fallthrough (delegates to ops/ methods)
# ---------------------------------------------------------------------------

static func _dispatch_headless(operation: String, params: Dictionary) -> Dictionary:
    ## Same dispatch table as daemon.gd — delegates to regular ops/ methods.
    match operation:
        "scene_create": return SceneOps.op_scene_create(params)
        "scene_read": return SceneOps.op_scene_read(params)
        "node_add": return NodeOps.op_node_add(params)
        "node_set_properties": return NodeOps.op_node_set_properties(params)
        "node_remove": return NodeOps.op_node_remove(params)
        "node_reparent": return NodeOps.op_node_reparent(params)
        "scene_list": return SceneOps.op_scene_list(params)
        "scene_add_instance": return SceneOps.op_scene_add_instance(params)
        "resource_read": return ResourceOps.op_resource_read(params)
        "material_create": return ResourceOps.op_material_create(params)
        "shape_create": return ResourceOps.op_shape_create(params)
        "style_box_create": return ResourceOps.op_style_box_create(params)
        "resource_duplicate": return ResourceOps.op_resource_duplicate(params)
        "tilemap_set_cells": return TileMapOps.op_tilemap_set_cells(params)
        "tilemap_get_cells": return TileMapOps.op_tilemap_get_cells(params)
        "tilemap_clear": return TileMapOps.op_tilemap_clear(params)
        "gridmap_set_cells": return GridMapOps.op_gridmap_set_cells(params)
        "gridmap_get_cells": return GridMapOps.op_gridmap_get_cells(params)
        "gridmap_clear": return GridMapOps.op_gridmap_clear(params)
        "animation_create": return AnimationOps.op_animation_create(params)
        "animation_add_track": return AnimationOps.op_animation_add_track(params)
        "animation_read": return AnimationOps.op_animation_read(params)
        "animation_remove_track": return AnimationOps.op_animation_remove_track(params)
        "ping":
            return {"success": true, "data": {"status": "ok", "backend": "editor"}, "operation": "ping"}
        _:
            return OpsUtil._error("Unknown operation: " + operation, operation, {})


# ---------------------------------------------------------------------------
# Post-operation editor sync
# ---------------------------------------------------------------------------

static func _post_operation_sync(operation: String, params: Dictionary, result: Dictionary) -> void:
    ## After a headless operation, sync the editor's state.
    ## - Reload open scenes that were modified on disk.
    ## - Scan filesystem for new/changed files.
    if not result.get("success", false):
        return

    var scene_path: String = params.get("scene_path", "")
    if scene_path != "":
        var full_path := "res://" + scene_path
        if full_path in EditorInterface.get_open_scenes():
            # Scene was modified on disk while open — reload it.
            EditorInterface.reload_scene_from_path(full_path)

    # Scan filesystem so new/modified files appear in FileSystem dock.
    EditorInterface.get_resource_filesystem().scan()


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

static func _resolve_node(scene_root: Node, path: String) -> Node:
    ## Resolve a node path relative to the scene root.
    ## Empty path returns the scene root itself.
    if path == "" or path == ".":
        return scene_root
    return scene_root.get_node_or_null(NodePath(path))


static func _set_owner_recursive(node: Node, owner: Node) -> void:
    ## Set owner for a node and all its descendants (needed for scene serialization).
    for child in node.get_children():
        child.owner = owner
        _set_owner_recursive(child, owner)
```

**Implementation Notes**:

- **Live vs headless dispatch**: The `dispatch()` function checks if the target
  scene is the currently active editor scene. If yes, it uses live tree
  manipulation (no disk I/O, sees unsaved state). If not, it delegates to the
  existing ops/ methods which load from disk.

- **Reusing ops/ code**: Live operations reuse helpers from the ops/ modules:
  - `SceneOps._serialize_node()` for tree serialization in `_live_scene_read`
  - `NodeOps._apply_properties()` for property setting with type conversion
  - `TileMapOps._set_cells_on_node()`, `_get_cells_from_node()`, `_clear_node()`
  - `GridMapOps._set_cells_on_node()`, `_get_cells_from_node()`, `_clear_node()`

  These helpers need to be extracted from the existing ops/ methods if they
  don't already exist as separate functions. The existing ops load scenes from
  disk and operate on instantiated trees — the core logic (adding nodes, setting
  properties, serializing) is the same; only the scene acquisition differs.

- **TileMap/GridMap node-level helpers**: The existing `tilemap_ops.gd` and
  `gridmap_ops.gd` bundle "load scene + find node + operate + save" into a
  single function. For live operations, we need the "operate on node" part
  split out. This requires adding `_set_cells_on_node()`,
  `_get_cells_from_node()`, and `_clear_node()` to each ops file, extracting
  the core logic from the existing `op_*` functions.

- **Post-operation sync**: After headless operations, `_post_operation_sync`
  reloads any open scenes that were modified on disk and scans the filesystem.
  This handles question 4 (auto-reload).

- **Node ownership**: When adding nodes live, `node.owner = scene_root` is
  critical. Without it, the node won't be serialized when the user saves the
  scene. Same for `_set_owner_recursive` when adding scene instances.

**Acceptance Criteria**:
- [ ] Operations on the active scene modify the live tree (no disk I/O)
- [ ] `_live_scene_read` returns the current in-memory state including unsaved changes
- [ ] `_live_node_add` creates a node in the live tree with correct owner
- [ ] `_live_node_set_properties` sets properties on live nodes with type conversion
- [ ] `_live_node_remove` removes a node from the live tree
- [ ] `_live_node_reparent` moves a node within the live tree
- [ ] `_live_scene_add_instance` instances a scene into the live tree
- [ ] Live tilemap/gridmap operations modify the live scene nodes
- [ ] Operations on non-active scenes delegate to ops/ methods
- [ ] After headless operations, open scenes are reloaded
- [ ] After headless operations, filesystem is scanned
- [ ] All 27 operations are handled (no unrecognized operation errors)

---

### Unit 7: Ops Module Refactoring (Helper Extraction)

The existing ops/ modules bundle scene loading + operation + saving into single
functions. Live editor operations need the "operation" part split out. This
unit extracts node-level helpers.

**File**: `addons/director/ops/scene_ops.gd`

Extract `_serialize_node()` as a static helper (if not already standalone):

```gdscript
## Serialize a node and its children to a Dictionary.
## Used by both op_scene_read (from disk) and EditorOps._live_scene_read (from live tree).
static func _serialize_node(node: Node, max_depth: int, current_depth: int,
        include_properties: bool) -> Dictionary:
    # ... existing serialization logic, unchanged ...
```

**File**: `addons/director/ops/node_ops.gd`

Extract `_apply_properties()` as a static helper:

```gdscript
## Apply properties to a node using type conversion.
## Returns an Array of property names that were successfully set.
static func _apply_properties(node: Node, properties: Dictionary) -> Array:
    var set_props: Array = []
    for key in properties:
        var value = properties[key]
        var converted = OpsUtil.convert_value_for_node(node, key, value)
        node.set(key, converted)
        set_props.append(key)
    return set_props
```

**File**: `addons/director/ops/tilemap_ops.gd`

Extract node-level helpers from existing `op_tilemap_*` functions:

```gdscript
## Set cells on an already-resolved TileMapLayer node.
## Called by both op_tilemap_set_cells (headless) and EditorOps (live).
static func _set_cells_on_node(node: Node, params: Dictionary) -> Dictionary:
    # Core cell-setting logic (currently inside op_tilemap_set_cells).
    # ...

## Read cells from an already-resolved TileMapLayer node.
static func _get_cells_from_node(node: Node, params: Dictionary) -> Dictionary:
    # ...

## Clear cells on an already-resolved TileMapLayer node.
static func _clear_node(node: Node, params: Dictionary) -> Dictionary:
    # ...
```

**File**: `addons/director/ops/gridmap_ops.gd`

Same pattern — extract `_set_cells_on_node()`, `_get_cells_from_node()`,
`_clear_node()`.

**Implementation Notes**:
- The existing `op_*` functions should be refactored to call these new helpers
  internally, keeping the external API unchanged.
- The helper functions take a resolved `Node` (no scene loading) and return
  the same `Dictionary` result format.
- This is a pure refactoring — no behavior changes for existing callers.

**Acceptance Criteria**:
- [ ] Existing ops/ tests still pass after extraction
- [ ] `SceneOps._serialize_node()` is callable with a live Node
- [ ] `NodeOps._apply_properties()` is callable with a live Node
- [ ] `TileMapOps._set_cells_on_node()` works on a live TileMapLayer
- [ ] `GridMapOps._set_cells_on_node()` works on a live GridMap
- [ ] No behavior changes for existing headless/daemon operations

---

### Unit 8: E2E Tests

**File**: `tests/director-tests/src/test_editor.rs`

Editor plugin tests use a mock editor server script that mimics the plugin's
TCP behavior in headless mode. This validates the full Rust TCP round-trip
without requiring the actual Godot editor.

**File**: `tests/director-tests/src/mock_editor_server.gd` (test helper,
deployed to the test project)

```gdscript
extends SceneTree

## Mock editor plugin server for testing.
## Runs headlessly with the same TCP protocol as plugin.gd,
## but delegates to regular ops/ (no EditorInterface available in headless).

const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
# ... same preloads as daemon.gd ...

var _server: TCPServer
var _client: StreamPeerTCP
var _read_buf: PackedByteArray = PackedByteArray()

func _init():
    var port := int(OS.get_environment("DIRECTOR_EDITOR_PORT")) \
        if OS.has_environment("DIRECTOR_EDITOR_PORT") else 6551
    _server = TCPServer.new()
    var err = _server.listen(port)
    if err != OK:
        printerr("Mock editor: failed to listen on port %d" % port)
        quit(1)
        return
    print(JSON.stringify({"source": "director", "status": "ready", "port": port, "backend": "mock_editor"}))

func _process(delta: float) -> bool:
    _accept_client()
    _poll_client()
    return false

# ... _accept_client, _poll_client, _try_decode_message, _send_message
# ... identical to daemon.gd TCP handling ...
# ... _dispatch identical to daemon.gd dispatch table ...
```

**File**: `tests/director-tests/src/harness.rs` — add `EditorFixture`

```rust
const EDITOR_DEFAULT_PORT: u16 = 16551; // offset from production port

/// Test fixture for the editor plugin backend.
///
/// Spawns a mock editor server (same protocol as plugin.gd but headless)
/// and connects via TCP. Tests the Rust EditorHandle and backend selection.
pub struct EditorFixture {
    child: Option<Child>,
    stream: Option<TcpStream>,
    port: u16,
    project_dir: PathBuf,
}

impl EditorFixture {
    pub fn start() -> Self {
        Self::start_with_port(EDITOR_DEFAULT_PORT)
    }

    pub fn start_with_port(port: u16) -> Self {
        // Same pattern as DaemonFixture but launches mock_editor_server.gd
        // on the editor port.
        // ...
    }

    pub fn run(&mut self, operation: &str, params: serde_json::Value)
        -> anyhow::Result<OperationResult>
    {
        // Same as DaemonFixture::run — length-prefixed JSON send/receive.
        // ...
    }
}

impl Drop for EditorFixture {
    fn drop(&mut self) {
        // Best-effort quit then kill, same as DaemonFixture.
    }
}
```

**File**: `tests/director-tests/src/test_editor.rs`

```rust
use serde_json::json;
use crate::harness::{EditorFixture, DirectorFixture};

#[test]
#[ignore = "requires Godot binary"]
fn editor_fixture_creates_and_reads_scene() {
    let mut e = EditorFixture::start();
    let scene_path = DirectorFixture::temp_scene_path("editor_create");

    let result = e.run("scene_create", json!({
        "scene_path": scene_path,
        "root_type": "Node2D",
    })).unwrap();
    result.unwrap_data();

    let read_result = e.run("scene_read", json!({
        "scene_path": scene_path,
    })).unwrap();
    let data = read_result.unwrap_data();
    assert_eq!(data["root"]["type"], "Node2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_fixture_node_add_and_set_properties() {
    let mut e = EditorFixture::start();
    let scene_path = DirectorFixture::temp_scene_path("editor_nodeadd");

    e.run("scene_create", json!({
        "scene_path": scene_path,
        "root_type": "Node2D",
    })).unwrap().unwrap_data();

    e.run("node_add", json!({
        "scene_path": scene_path,
        "node_type": "Sprite2D",
        "node_name": "TestSprite",
    })).unwrap().unwrap_data();

    let read = e.run("scene_read", json!({
        "scene_path": scene_path,
    })).unwrap().unwrap_data();

    let children = read["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "TestSprite");
    assert_eq!(children[0]["type"], "Sprite2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_ping_returns_editor_backend() {
    let mut e = EditorFixture::start();
    let result = e.run("ping", json!({})).unwrap().unwrap_data();
    assert_eq!(result["backend"], "editor");
}
```

**Implementation Notes**:
- The mock editor server uses the same TCP protocol and operation dispatch as
  the real plugin, but runs headlessly. This tests the Rust TCP client code
  and backend selection without the actual Godot editor.
- `EditorFixture` mirrors `DaemonFixture` in structure — same spawn pattern,
  same stdout drain, same length-prefixed TCP I/O.
- Test port is offset from production (16551 vs 6551) to avoid conflicts.
- The `ping` test verifies the `"backend": "editor"` field, confirming the
  response came from the editor server rather than the daemon.
- Testing actual EditorInterface behavior (live scene tree, dirty state,
  reload) requires manual testing in the Godot editor — not automatable in
  headless E2E tests. The design doc should note this.

**Acceptance Criteria**:
- [ ] `EditorFixture` starts mock editor server and connects via TCP
- [ ] Operations round-trip through mock editor server correctly
- [ ] `ping` returns `"backend": "editor"`
- [ ] All existing director tests continue to pass
- [ ] Mock editor server handles all 27 operations

---

### Unit 9: Backend Selection Integration Test

**File**: `tests/director-tests/src/test_editor.rs` (additional tests)

```rust
#[test]
#[ignore = "requires Godot binary"]
fn backend_prefers_editor_over_daemon() {
    // Start both an editor fixture (port 16551) and daemon fixture (port 16550).
    // Verify that with both running, the Backend struct routes to the editor.
    //
    // This test validates the priority logic in Backend::run_operation.
    // It uses the Rust Backend struct directly rather than going through
    // the MCP server.

    let _editor = EditorFixture::start_with_port(16551);
    let _daemon = DaemonFixture::start_with_port(16550);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let backend = Backend::new();
        let godot = resolve_godot_bin().unwrap();
        let project = project_dir_path();
        let scene = DirectorFixture::temp_scene_path("backend_priority");

        // Set env vars so Backend finds the test ports.
        unsafe {
            std::env::set_var("DIRECTOR_EDITOR_PORT", "16551");
            std::env::set_var("DIRECTOR_DAEMON_PORT", "16550");
        }

        let result = backend
            .run_operation(&godot, &project, "scene_create", &json!({
                "scene_path": scene,
                "root_type": "Node2D",
            }))
            .await
            .unwrap();

        assert!(result.success);

        // Ping to verify we're hitting the editor backend.
        let ping = backend
            .run_operation(&godot, &project, "ping", &json!({}))
            .await
            .unwrap();
        assert_eq!(ping.data["backend"], "editor");

        // Cleanup env vars.
        unsafe {
            std::env::remove_var("DIRECTOR_EDITOR_PORT");
            std::env::remove_var("DIRECTOR_DAEMON_PORT");
        }

        backend.shutdown().await;
    });
}

#[test]
#[ignore = "requires Godot binary"]
fn backend_falls_through_to_daemon_when_no_editor() {
    // Only start a daemon fixture. Verify Backend falls through to daemon
    // when no editor is available.
    let _daemon = DaemonFixture::start_with_port(16550);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let backend = Backend::new();
        let godot = resolve_godot_bin().unwrap();
        let project = project_dir_path();

        unsafe {
            std::env::set_var("DIRECTOR_EDITOR_PORT", "16599"); // nothing listening
            std::env::set_var("DIRECTOR_DAEMON_PORT", "16550");
        }

        let result = backend
            .run_operation(&godot, &project, "ping", &json!({}))
            .await
            .unwrap();

        assert!(result.success);
        // Daemon ping doesn't have "backend" field — absence confirms it's daemon.
        assert!(result.data.get("backend").is_none());

        unsafe {
            std::env::remove_var("DIRECTOR_EDITOR_PORT");
            std::env::remove_var("DIRECTOR_DAEMON_PORT");
        }

        backend.shutdown().await;
    });
}
```

**Acceptance Criteria**:
- [ ] Backend routes to editor when both editor and daemon are available
- [ ] Backend falls through to daemon when editor is unreachable
- [ ] Backend falls through to one-shot when neither editor nor daemon is available

---

## Implementation Order

1. **Unit 7: Ops module refactoring** — Extract node-level helpers from
   tilemap_ops.gd, gridmap_ops.gd, node_ops.gd, scene_ops.gd. Pure
   refactoring, no behavior changes. Must pass existing tests.

2. **Unit 1: EditorHandle** — Rust TCP client + port resolution + unit tests.
   Self-contained, testable without Godot.

3. **Unit 3: Error type registration** — `From<EditorError>` impls.

4. **Unit 4: Module registration** — `pub mod editor` in lib.rs.

5. **Unit 2: Backend selection update** — Add editor priority to backend.rs.
   Depends on Unit 1.

6. **Unit 6: editor_ops.gd** — Editor operations dispatcher. Depends on
   Unit 7 (helper extraction).

7. **Unit 5: plugin.gd** — Editor plugin TCP listener. Depends on Unit 6.

8. **Unit 8: E2E tests** — Mock editor fixture + round-trip tests. Depends
   on Units 5 and 2.

9. **Unit 9: Backend selection integration tests** — Depends on Unit 8.

---

## Testing

### Unit Tests: `crates/director/src/editor.rs`

- `resolve_default_port` — returns 6551 with no env var or project.godot
- `resolve_env_var_port` — returns env var value
- `parse_project_godot_port` — parses `[director]` section
- `parse_project_godot_no_section` — returns None when no `[director]` section
- `parse_project_godot_wrong_section` — returns None for wrong section name

### E2E Tests: `tests/director-tests/src/test_editor.rs`

- `editor_fixture_creates_and_reads_scene` — create + read via mock editor
- `editor_fixture_node_add_and_set_properties` — node manipulation via editor
- `editor_ping_returns_editor_backend` — verifies `"backend": "editor"` in ping
- `backend_prefers_editor_over_daemon` — priority test with both backends
- `backend_falls_through_to_daemon_when_no_editor` — fallthrough test

### Manual Testing Checklist (requires Godot editor)

- [ ] Enable Director plugin in Godot editor → plugin listens on port
- [ ] Run `director ping` from CLI → returns `"backend": "editor"`
- [ ] Create a scene via Director while editor is open → scene appears in FileSystem dock
- [ ] Modify the active scene via Director → changes appear live in viewport
- [ ] Read the active scene with unsaved changes → response includes dirty state
- [ ] Close Godot editor → Director falls through to daemon/one-shot
- [ ] Custom port via project setting → plugin listens on custom port

---

## Verification Checklist

```bash
# Build
cargo build -p director

# Unit tests
cargo test -p director

# E2E tests (requires Godot binary + test project deployed)
theatre-deploy ~/dev/stage/tests/godot-project
cargo test -p director-tests -- --include-ignored

# Lint
cargo clippy -p director
cargo fmt --check

# Verify existing tests still pass
cargo test --workspace
```
