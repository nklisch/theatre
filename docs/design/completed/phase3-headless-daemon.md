# Design: Director Phase 3 — Headless Daemon

## Overview

Phase 3 adds a persistent headless Godot process (the "daemon") that eliminates
the 1-3s cold-start cost per operation. Instead of spawning a fresh Godot
process for every tool call, a single Godot instance stays alive with a TCP
command server. Subsequent operations complete in ~50ms.

The daemon is transparent to MCP tool handlers — `backend.rs` selects the
fastest available backend automatically.

### Decisions (resolved during design)

| Decision | Choice |
|---|---|
| Multi-project | Single project at a time. Switching projects kills the old daemon, spawns new. |
| Shutdown trigger | Both: idle timeout (default 5 min) + quit on MCP server exit |
| Ready signal | Line-scan stdout for JSON with `"source": "director"` identifier field |
| Concurrency | Strict serial — one request in flight at a time, mutex-guarded |
| Port | `:6550` (hardcoded default, configurable via `DIRECTOR_DAEMON_PORT` env var) |

---

## Implementation Units

### Unit 1: Daemon GDScript (`addons/director/daemon.gd`)

**File**: `addons/director/daemon.gd`

```gdscript
extends SceneTree

## Director headless daemon — persistent TCP command server.
## Launched via: godot --headless --path <project> --script addons/director/daemon.gd

const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")

const DEFAULT_PORT := 6550
const IDLE_TIMEOUT_SEC := 300  # 5 minutes

var _server: TCPServer
var _client: StreamPeerTCP
var _last_activity_time: float
var _port: int

func _init():
    ...

func _process(delta: float) -> void:
    # Accept connections, read commands, check idle timeout
    ...

func _accept_client() -> void: ...
func _poll_client() -> void: ...
func _read_message() -> Dictionary: ...  # length-prefixed JSON
func _send_message(data: Dictionary) -> void: ...  # length-prefixed JSON
func _dispatch(operation: String, params: Dictionary) -> Dictionary: ...
func _check_idle_timeout() -> void: ...
```

**Wire protocol**: Length-prefixed JSON, matching stage-protocol's framing:
`[4 bytes: big-endian u32 length][JSON payload, UTF-8]`

**Request format** (from Rust → GDScript):
```json
{"operation": "scene_read", "params": {"scene_path": "scenes/player.tscn"}}
```

**Response format** (from GDScript → Rust):
```json
{"success": true, "data": {...}, "operation": "scene_read"}
```
Same `OperationResult` shape as one-shot stdout output.

**Special operations**:
- `{"operation": "quit"}` → daemon prints `{"source":"director","status":"shutdown"}` to stdout, calls `quit(0)`
- `{"operation": "ping"}` → returns `{"success": true, "data": {"status": "ok"}}` (health check)

**Ready signal** (printed to stdout on successful TCP bind):
```json
{"source": "director", "status": "ready", "port": 6550}
```

**Idle timeout**: After `IDLE_TIMEOUT_SEC` with no connected client and no
operations, daemon prints `{"source":"director","status":"idle_shutdown"}` to
stdout and calls `quit(0)`.

**Port resolution**: `DIRECTOR_DAEMON_PORT` env var → default `6550`.

**Dispatch**: Reuses the exact same operation match as `operations.gd`:
```gdscript
func _dispatch(operation: String, params: Dictionary) -> Dictionary:
    match operation:
        "scene_create": return SceneOps.op_scene_create(params)
        "scene_read": return SceneOps.op_scene_read(params)
        # ... identical to operations.gd match block
        "quit": quit(0); return {}
        "ping": return {"success": true, "data": {"status": "ok"}}
        _: return {"success": false, "error": "Unknown operation: " + operation, ...}
```

**Implementation Notes**:
- TCP server accepts one client at a time. If a second client connects, the
  first is disconnected (simplifies lifecycle — the Rust daemon manager is the
  only expected client).
- Uses `TCPServer` and `StreamPeerTCP` (Godot built-in, no GDExtension needed).
- Length-prefix framing: read 4 bytes as big-endian u32, then read that many
  bytes of JSON. Same for writes. This matches stage-protocol's codec.
- `_process` is used (not `_physics_process`) because daemon does not interact
  with the physics simulation.
- The idle timeout timer resets on: client connect, client disconnect, any
  operation received. It does NOT reset on ping.

**Acceptance Criteria**:
- [ ] Daemon starts, binds to configured port, prints ready JSON to stdout
- [ ] Accepts TCP client, receives length-prefixed JSON commands
- [ ] Dispatches all operations identical to `operations.gd`
- [ ] Returns length-prefixed JSON responses
- [ ] Responds to `ping` with `{"success": true, "data": {"status": "ok"}}`
- [ ] Responds to `quit` by exiting cleanly
- [ ] Shuts down after 5 minutes of idle (no client, no operations)
- [ ] Prints `{"source":"director","status":"idle_shutdown"}` on idle exit

---

### Unit 2: Daemon Client (`crates/director/src/daemon.rs`)

**File**: `crates/director/src/daemon.rs`

```rust
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Child;

use crate::oneshot::{OperationError, OperationResult};

const DEFAULT_PORT: u16 = 6550;
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages a single headless Godot daemon process.
pub struct DaemonHandle {
    child: Child,
    stream: TcpStream,
    project_path: PathBuf,
    port: u16,
}

/// Errors specific to daemon lifecycle.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("daemon failed to start: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("daemon did not become ready within {0:?}")]
    ReadyTimeout(Duration),

    #[error("daemon TCP connection failed: {0}")]
    ConnectionFailed(#[source] std::io::Error),

    #[error("daemon TCP I/O error: {0}")]
    IoError(#[source] std::io::Error),

    #[error("daemon response parse error: {source}\nraw: {raw}")]
    ParseFailed {
        #[source]
        source: serde_json::Error,
        raw: String,
    },

    #[error("daemon process exited unexpectedly")]
    ProcessExited,
}

impl DaemonHandle {
    /// Spawn a new daemon for the given project.
    ///
    /// Launches `godot --headless --path <project> --script daemon.gd`,
    /// waits for the `{"source":"director","status":"ready"}` signal on
    /// stdout, then connects via TCP.
    pub async fn spawn(
        godot_bin: &Path,
        project_path: &Path,
        port: u16,
    ) -> Result<Self, DaemonError> { ... }

    /// Send an operation to the daemon and return the result.
    ///
    /// Wire format: length-prefixed JSON (4-byte BE u32 + JSON payload).
    pub async fn send_operation(
        &mut self,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, DaemonError> { ... }

    /// Send quit command and wait for process exit.
    pub async fn shutdown(mut self) -> Result<(), DaemonError> { ... }

    /// Check if the daemon process is still running.
    pub fn is_alive(&mut self) -> bool { ... }

    /// The project path this daemon was spawned for.
    pub fn project_path(&self) -> &Path { ... }
}

/// Resolve the daemon port from env var or default.
pub fn resolve_daemon_port() -> u16 {
    std::env::var("DIRECTOR_DAEMON_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}
```

**Length-prefix codec** (private helper functions in this module):
```rust
/// Write a length-prefixed JSON message to a TCP stream.
async fn write_message(
    stream: &mut TcpStream,
    value: &serde_json::Value,
) -> Result<(), DaemonError> {
    let json = serde_json::to_vec(value)
        .map_err(|e| DaemonError::ParseFailed { source: e, raw: String::new() })?;
    let len = (json.len() as u32).to_be_bytes();
    stream.write_all(&len).await.map_err(DaemonError::IoError)?;
    stream.write_all(&json).await.map_err(DaemonError::IoError)?;
    stream.flush().await.map_err(DaemonError::IoError)?;
    Ok(())
}

/// Read a length-prefixed JSON message from a TCP stream.
async fn read_message(stream: &mut TcpStream) -> Result<serde_json::Value, DaemonError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(DaemonError::IoError)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await.map_err(DaemonError::IoError)?;
    let raw = String::from_utf8_lossy(&buf).into_owned();
    serde_json::from_slice(&buf).map_err(|source| DaemonError::ParseFailed { source, raw })
}
```

**Spawn sequence**:
1. Set `DIRECTOR_DAEMON_PORT` env var on the child process
2. Spawn `godot --headless --path <project> --script addons/director/daemon.gd`
3. Read stdout line-by-line, JSON-parse each, look for
   `{"source":"director","status":"ready","port":N}`
4. Timeout after `READY_TIMEOUT` (15s) if not found
5. Connect to `localhost:<port>` via TCP
6. Return `DaemonHandle`

**Implementation Notes**:
- `DaemonHandle` owns the `Child` process. On `Drop`, if `shutdown()` was not
  called, the child is killed (best-effort cleanup).
- `send_operation` wraps the request as
  `{"operation":"<name>","params":{...}}`, writes length-prefixed JSON, reads
  length-prefixed JSON response, parses as `OperationResult`.
- The 30s operation timeout uses `tokio::time::timeout` around the
  read, matching oneshot behavior.
- `is_alive` calls `child.try_wait()` to check if process has exited.

**Acceptance Criteria**:
- [ ] `DaemonHandle::spawn` launches Godot daemon and waits for ready signal
- [ ] `send_operation` sends length-prefixed JSON and receives response
- [ ] `shutdown` sends quit and waits for process exit
- [ ] `is_alive` correctly reports process state
- [ ] `resolve_daemon_port` reads `DIRECTOR_DAEMON_PORT` env var
- [ ] Timeout after 15s if daemon never becomes ready
- [ ] Timeout after 30s if operation never responds

---

### Unit 3: Backend Router (`crates/director/src/backend.rs`)

**File**: `crates/director/src/backend.rs`

```rust
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::daemon::{DaemonError, DaemonHandle, resolve_daemon_port};
use crate::oneshot::{self, OperationError, OperationResult};
use crate::resolve::resolve_godot_bin;

/// Backend selection: daemon → one-shot fallback.
///
/// Phase 3 implements two backends. Phase 7 will add editor plugin (`:6551`)
/// as highest priority.
pub struct Backend {
    daemon: Mutex<Option<DaemonHandle>>,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            daemon: Mutex::new(None),
        }
    }

    /// Run an operation via the best available backend.
    ///
    /// Tries daemon first; if daemon fails or is for a different project,
    /// falls back to one-shot. On daemon connection failure, attempts one
    /// respawn before falling back.
    pub async fn run_operation(
        &self,
        godot_bin: &Path,
        project_path: &Path,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<OperationResult, OperationError> { ... }

    /// Shut down any running daemon.
    pub async fn shutdown(&self) { ... }
}
```

**Selection logic** (inside `run_operation`):
```
1. Lock daemon mutex
2. If daemon exists AND daemon.project_path() == project_path AND daemon.is_alive():
   a. Try send_operation
   b. On success → return result
   c. On DaemonError (connection lost):
      - Kill old daemon
      - Try respawn (step 3)
      - If respawn fails → fall through to one-shot (step 4)
3. If no daemon OR project mismatch:
   a. Shut down existing daemon if any
   b. Try DaemonHandle::spawn(godot_bin, project_path, port)
   c. On success → send_operation, return result
   d. On DaemonError → fall through to one-shot (step 4)
4. One-shot fallback:
   run_oneshot(godot_bin, project_path, operation, params).await
```

**Project switching**: If `project_path` differs from the daemon's bound
project, the existing daemon is shut down and a new one spawned.

**Respawn-once**: On connection failure during `send_operation`, the daemon is
killed and respawned exactly once. If the respawn also fails, fall back to
one-shot for this call (daemon stays dead until next call triggers a fresh
spawn attempt).

**DaemonError → OperationError conversion**:
```rust
impl From<DaemonError> for OperationError {
    fn from(e: DaemonError) -> Self {
        OperationError::ProcessFailed {
            status: -1,
            stderr: e.to_string(),
        }
    }
}
```

**Implementation Notes**:
- `Backend` uses `tokio::sync::Mutex` (not `std::sync::Mutex`) because the
  lock is held across `.await` points (daemon spawn + send are async).
- The mutex serializes all daemon operations — strict serial concurrency as
  decided.
- `Backend` is stored in `DirectorServer` behind `Arc` so it can be shared
  across tool handler invocations.
- One-shot fallback is always available — the daemon is an optimization, not a
  requirement.

**Acceptance Criteria**:
- [ ] Uses daemon when available, falls back to one-shot
- [ ] Respawns daemon once on connection failure
- [ ] Switches projects by killing old daemon and spawning new
- [ ] Falls back to one-shot if daemon spawn fails
- [ ] Serializes concurrent operations via mutex
- [ ] `shutdown()` sends quit to daemon

---

### Unit 4: Server + MCP Integration

**File**: `crates/director/src/server.rs` (modify existing)

```rust
use std::sync::Arc;
use crate::backend::Backend;

#[derive(Clone)]
pub struct DirectorServer {
    pub tool_router: ToolRouter<Self>,
    pub backend: Arc<Backend>,
}

impl DirectorServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            backend: Arc::new(Backend::new()),
        }
    }
}
```

**File**: `crates/director/src/mcp/mod.rs` (modify `run_operation`)

```rust
/// Run an operation via the best available backend.
/// Handles godot resolution, project validation, and backend routing.
async fn run_operation(
    backend: &Backend,
    project_path: &str,
    operation: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    let godot = resolve_godot_bin().map_err(McpError::from)?;
    let project = std::path::Path::new(project_path);
    validate_project_path(project).map_err(McpError::from)?;

    let result = backend
        .run_operation(&godot, project, operation, params)
        .await
        .map_err(McpError::from)?;

    result.into_data().map_err(McpError::from)
}
```

**Tool handlers** change from:
```rust
let data = run_operation(&params.project_path, "scene_create", &op_params).await?;
```
to:
```rust
let data = run_operation(&self.backend, &params.project_path, "scene_create", &op_params).await?;
```

All 8 existing tool handlers get this mechanical update — pass `&self.backend`
as first argument to `run_operation`.

**File**: `crates/director/src/main.rs` (add shutdown hook)

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // ... existing setup ...
    let server = DirectorServer::new();
    let backend = Arc::clone(&server.backend);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    backend.shutdown().await;
    tracing::info!("MCP session ended, shutting down");
    Ok(())
}
```

**File**: `crates/director/src/lib.rs` (add module)

```rust
pub mod backend;
pub mod daemon;
pub mod error;
pub mod mcp;
pub mod oneshot;
pub mod resolve;
pub mod server;
```

**Implementation Notes**:
- `DirectorServer` gains an `Arc<Backend>` field. Since `DirectorServer` is
  `Clone` (required by rmcp), and `Backend` contains a `tokio::sync::Mutex`,
  `Arc` is needed.
- The `run_operation` helper in `mcp/mod.rs` changes signature but all call
  sites update mechanically.
- `main.rs` calls `backend.shutdown()` after the MCP service ends, sending
  quit to any running daemon.

**Acceptance Criteria**:
- [ ] `DirectorServer` holds `Arc<Backend>`
- [ ] All 8 tool handlers pass backend to `run_operation`
- [ ] `run_operation` routes through `Backend` instead of `run_oneshot` directly
- [ ] `main.rs` shuts down backend on exit
- [ ] Existing one-shot behavior is preserved when no daemon is available

---

### Unit 5: Error Conversions

**File**: `crates/director/src/error.rs` (extend existing)

```rust
use crate::daemon::DaemonError;

/// Convert DaemonError to McpError for use in tool handlers.
impl From<DaemonError> for McpError {
    fn from(e: DaemonError) -> Self {
        McpError::internal_error(e.to_string(), None)
    }
}
```

This addition is mechanical — follows the same pattern as the existing
`OperationError` and `ResolveError` conversions.

**Acceptance Criteria**:
- [ ] `DaemonError` converts to `McpError::internal_error`

---

### Unit 6: Cargo.toml Dependency Update

**File**: `crates/director/Cargo.toml`

No new crate dependencies needed. `tokio` (already included) provides
`TcpStream`, `process::Command`, `sync::Mutex`, `time::timeout`, and
`io::{AsyncReadExt, AsyncWriteExt}`. The `tokio` workspace dependency already
includes `full` features.

No changes to `tests/director-tests/Cargo.toml` either — E2E tests use the
existing fixture pattern (which goes through the MCP layer or directly spawns
Godot).

**Acceptance Criteria**:
- [ ] `cargo build -p director` succeeds with no new dependencies

---

### Unit 7: E2E Tests

**File**: `tests/director-tests/src/test_daemon.rs`

```rust
use crate::harness::DirectorFixture;
use serde_json::json;

/// Test that daemon launches, accepts operations, and shuts down.
#[test]
#[ignore = "requires Godot binary"]
fn daemon_lifecycle() {
    // 1. Start daemon
    // 2. Send ping, verify response
    // 3. Send scene_create, verify success
    // 4. Send scene_read, verify round-trip
    // 5. Send quit, verify clean exit
}

/// Test that daemon idle timeout works.
#[test]
#[ignore = "requires Godot binary"]
fn daemon_idle_timeout() {
    // 1. Start daemon with short idle timeout (env var or param)
    // 2. Wait for timeout + margin
    // 3. Verify process has exited
}

/// Test that backend falls back to one-shot when daemon is unavailable.
#[test]
#[ignore = "requires Godot binary"]
fn fallback_to_oneshot() {
    // 1. Run operation without starting daemon
    // 2. Verify operation succeeds (one-shot)
}
```

The daemon E2E tests need a direct `DaemonHandle` test path, since the
`DirectorFixture` currently only tests one-shot. Two approaches:

**Option A**: Add `DaemonFixture` to `harness.rs` that spawns and manages the
daemon directly:
```rust
pub struct DaemonFixture {
    handle: DaemonHandle,
}

impl DaemonFixture {
    pub fn start() -> Self { ... }
    pub fn run(&mut self, operation: &str, params: Value) -> OperationResult { ... }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) { /* send quit */ }
}
```

**Option B**: Test through the `Backend` abstraction — create a `Backend`,
call `run_operation`, and verify it spawns a daemon automatically.

Recommend **Option A** for unit-level daemon testing (verifies the TCP protocol
works) and **Option B** for integration testing (verifies backend selection).

**File**: `tests/director-tests/src/harness.rs` (extend)

```rust
/// A daemon runner for E2E tests.
pub struct DaemonFixture {
    godot_bin: String,
    project_dir: PathBuf,
    child: Option<std::process::Child>,
    stream: Option<std::net::TcpStream>,
    port: u16,
}

impl DaemonFixture {
    pub fn start() -> Self { ... }
    pub fn start_with_port(port: u16) -> Self { ... }

    /// Send an operation via length-prefixed JSON and read the response.
    pub fn run(&mut self, operation: &str, params: serde_json::Value)
        -> anyhow::Result<OperationResult> { ... }

    /// Send quit command.
    pub fn quit(&mut self) -> anyhow::Result<()> { ... }

    fn project_dir() -> PathBuf { ... }  // same as DirectorFixture
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        // Best-effort quit, then kill
    }
}
```

**Note**: `DaemonFixture` uses synchronous `std::net::TcpStream` (not tokio)
because the test harness is synchronous (matching `DirectorFixture` pattern).
Length-prefix read/write helpers are duplicated as sync versions in the test
harness.

**File**: `tests/director-tests/src/lib.rs` (add module)

```rust
mod harness;
mod test_daemon;
// ... existing modules
```

**Acceptance Criteria**:
- [ ] `daemon_lifecycle` test: spawn → ping → operation → quit
- [ ] `daemon_idle_timeout` test: spawn → wait → verify exit
- [ ] `fallback_to_oneshot` test: operation succeeds without daemon
- [ ] All existing Phase 1 + Phase 2 tests still pass unchanged

---

## Implementation Order

1. **Unit 1: `daemon.gd`** — The GDScript daemon must exist before anything
   can test against it. No Rust dependencies.

2. **Unit 2: `daemon.rs`** — Rust client for the daemon. Depends on Unit 1
   for the running daemon process.

3. **Unit 5: Error conversions** — Small addition, needed before Unit 3 can
   compile (backend needs `DaemonError → OperationError` conversion).

4. **Unit 3: `backend.rs`** — Backend router. Depends on Units 2 and 5.

5. **Unit 4: Server + MCP integration** — Wire backend into existing server
   and tool handlers. Depends on Unit 3.

6. **Unit 6: Cargo.toml** — Verify no new deps needed (should be a no-op).

7. **Unit 7: E2E tests** — Test the full stack. Depends on all above.

---

## Testing

### Unit Tests (inline in source files)

**`crates/director/src/daemon.rs`**:
- `test_resolve_daemon_port_default` — returns 6550 when env var unset
- `test_resolve_daemon_port_from_env` — reads `DIRECTOR_DAEMON_PORT`

**`crates/director/src/backend.rs`**:
- No meaningful unit tests — backend logic is integration-level (requires
  real Godot process). Covered by E2E tests.

### E2E Tests (`tests/director-tests/`)

**`test_daemon.rs`** (all `#[ignore = "requires Godot binary"]`):
- `daemon_lifecycle` — full spawn → ping → operations → quit cycle
- `daemon_idle_timeout` — verify idle shutdown (use short timeout via env var)
- `daemon_operations_match_oneshot` — run same operation via both daemon and
  one-shot, verify identical results
- `daemon_respawn_on_crash` — kill daemon process, verify next operation
  still succeeds (via Backend respawn logic, or manual test)
- `fallback_to_oneshot` — verify one-shot works when daemon is unavailable

### Existing Test Preservation

All existing tests in `test_scene.rs`, `test_node.rs`, `test_journey.rs`,
etc. continue to work unchanged — they use `DirectorFixture` which invokes
one-shot directly. The Backend integration doesn't affect these.

---

## Verification Checklist

```bash
# Build
cargo build -p director

# Clippy
cargo clippy -p director

# Unit tests (no Godot needed)
cargo test -p director

# E2E tests (requires Godot)
cargo test -p director-tests -- --include-ignored

# Verify daemon starts manually
godot --headless --path tests/godot-project --script addons/director/daemon.gd
# Expected: {"source":"director","status":"ready","port":6550} on stdout

# Verify all workspace tests still pass
theatre-deploy ~/dev/stage/tests/godot-project
cargo test --workspace
```
