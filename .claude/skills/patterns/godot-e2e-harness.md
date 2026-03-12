# Pattern: Godot E2E Test Harness

E2E tests use fixture structs that manage a live Godot process and a TCP
connection. Two harness types exist: `GodotFixture` (wire-protocol tests) and
`DirectorFixture`/`DaemonFixture` (scene-editor tests).

## Rationale

E2E tests need a live Godot process with the GDExtension loaded. The fixture
handles process launch, port selection, handshake completion, and automatic
cleanup via `Drop`. Tests just call `query(method, params)` or
`run(operation, params)` without managing lifecycle.

---

## GodotFixture (Wire Tests)

Tests the TCP wire protocol directly — query/response against the GDExtension.

**File**: `tests/wire-tests/src/harness.rs:13`

### Usage
```rust
#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn snapshot_returns_entities() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let data = f.query("get_snapshot_data", json!({
        "perspective": { "type": "camera" },
        "radius": 100.0,
    }))
    .unwrap()
    .unwrap_data();

    let entities = data["entities"].as_array().unwrap();
    assert!(!entities.is_empty());
}
```

### Structure
```rust
pub struct GodotFixture {
    child: Option<Child>,
    pub port: u16,
    stream: TcpStream,
    pub handshake: Handshake,
}

impl GodotFixture {
    // Picks free port, spawns Godot headless, waits for TCP, completes handshake
    pub fn start(scene: &str) -> anyhow::Result<Self> { ... }

    // Send Message::Query, receive Message::Response — synchronous
    pub fn query(&mut self, method: &str, params: serde_json::Value) -> anyhow::Result<QueryResult> { ... }

    // Disconnect without killing Godot (for reconnect resilience tests)
    pub fn disconnect_keep_alive(mut self) -> (u16, Child) { ... }
}

impl Drop for GodotFixture {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
```

### QueryResult
```rust
pub enum QueryResult {
    Ok(serde_json::Value),
    Err { code: String, message: String },
}

// Panics with message on wrong variant — test-friendly
result.unwrap_data()   // panics if Err
result.unwrap_err()    // panics if Ok
result.is_ok() / result.is_err()
```

### Shared helpers
```rust
// Float comparison within 0.01
pub fn assert_approx(actual: f64, expected: f64) { ... }

// Find entity by path fragment in snapshot response
pub fn find_entity<'a>(data: &'a serde_json::Value, name: &str) -> &'a serde_json::Value { ... }
```

---

## TestHarness (Integration Tests — Mock TCP)

Connects a real `SpectatorServer` to a `MockAddon` (fake Godot) over an in-process TCP socket. Used for integration tests that need a real server but not a real Godot process.

**File**: `crates/spectator-server/tests/support/harness.rs:15`

### Usage
```rust
#[tokio::test]
async fn snapshot_returns_budget() {
    let handler: QueryHandler = Arc::new(|_method, _params| Ok(mock_scene_3d()));
    let h = TestHarness::new(handler).await;

    let result = h.call_tool("spatial_snapshot", json!({"detail": "summary"})).await.unwrap();
    assert!(result["budget"].is_object());
}
```

### Structure
```rust
pub struct TestHarness {
    pub server: SpectatorServer,
    pub mock: MockAddon,
    pub state: Arc<Mutex<SessionState>>,
    _tcp_task: JoinHandle<()>,   // aborted on Drop
}

impl TestHarness {
    pub async fn new(handler: QueryHandler) -> Self { ... }      // 3D handshake
    pub async fn new_2d(handler: QueryHandler) -> Self { ... }   // 2D handshake
    pub async fn call_tool(&self, name: &str, params: Value) -> Result<Value, McpError> { ... }
    pub async fn call_tool_raw(&self, name: &str, params: Value) -> Result<String, McpError> { ... }
}
```

### MockAddon
```rust
pub type QueryHandler = Arc<dyn Fn(&str, &Value) -> Result<Value, (String, String)> + Send + Sync>;

pub struct MockAddon {
    pub port: u16,
    ...
}

impl MockAddon {
    pub async fn start(handler: QueryHandler) -> Self { ... }
    pub async fn push_event(&self, event: &str, data: Value) { ... }   // inject push events
}
```

---

## E2EHarness (Full-Stack E2E — Numbered Steps with Trace)

Wraps `GodotProcess` + `SpectatorServer` with a numbered-step API and automatic trace output on failure. Used for multi-step journey tests that verify real Godot behavior.

**File**: `crates/spectator-server/tests/support/e2e_harness.rs:16`

### Usage
```rust
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn journey_explore_scene() {
    let mut h = E2EHarness::start_3d().await.expect("Failed to start Godot");

    // Step 1: check connection established
    assert!(h.state.lock().await.connected);

    // Step 2: success expected — panics with trace_dump on failure
    let tree = h.expect(2, "scene_tree", json!({"action": "roots"})).await;
    assert!(tree["roots"].as_array().is_some());

    // Step 3: error expected — panics with trace_dump on success
    h.expect_err(3, "spatial_inspect", json!({"node": "nonexistent"})).await;

    // Step 4: wait for physics frames
    h.wait_frames(5).await;
}
```

### Step methods
```rust
// Returns Result — use when error is a valid outcome
pub async fn step(&mut self, n: usize, tool: &str, params: Value)
    -> Result<Value, McpError> { ... }

// Panics with trace_dump on error — use for "must succeed" steps
pub async fn expect(&mut self, n: usize, tool: &str, params: Value)
    -> Value { ... }

// Panics with trace_dump on success — use for "must fail" steps
pub async fn expect_err(&mut self, n: usize, tool: &str, params: Value)
    -> McpError { ... }

// For tools returning mixed content (text + images)
pub async fn expect_result(&mut self, n: usize, tool: &str, params: Value)
    -> CallToolResult { ... }

// At --fixed-fps 60: waits n * 1000/60 + 50ms
pub async fn wait_frames(&mut self, n: u32) { ... }
```

### trace_dump on panic
When `expect`/`expect_err` panics, the error message includes all prior steps with tool, params, result, and elapsed_ms, plus the last 20 lines of Godot stderr. This means every E2E failure is fully self-describing.

---

## DirectorFixture (Director Tests — One-Shot)

Spawns a fresh Godot process per operation; parses JSON result from stdout.

**File**: `tests/director-tests/src/harness.rs:10`

### Usage
```rust
#[test]
#[ignore = "requires Godot binary"]
fn create_enemy_scene() {
    let f = DirectorFixture::new();
    let scene_path = DirectorFixture::temp_scene_path("test_enemy");

    let result = f.run("scene_create", json!({
        "path": scene_path,
        "root_name": "EnemyRoot",
        "root_type": "CharacterBody2D",
    })).unwrap();

    result.unwrap_data();
}
```

### Structure
```rust
pub struct DirectorFixture {
    godot_bin: String,
    project_dir: PathBuf,
}

impl DirectorFixture {
    pub fn new() -> Self { ... }

    // Each call spawns godot --headless --script addons/director/operations.gd -- <op> '<json>'
    pub fn run(&self, operation: &str, params: serde_json::Value) -> anyhow::Result<OperationResult> { ... }

    // Returns "tmp/test_<name>.tscn" — use for test isolation
    pub fn temp_scene_path(name: &str) -> String { ... }
}

pub struct OperationResult {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

// Panics on wrong variant
result.unwrap_data()
result.unwrap_err() -> String
```

---

## DaemonFixture (Director Tests — Long-Running)

Persistent daemon across multiple operations; TCP connection shared.

**File**: `tests/director-tests/src/harness.rs:128`

### Usage
```rust
#[test]
#[ignore = "requires Godot binary"]
fn daemon_creates_and_instantiates() {
    let mut d = DaemonFixture::start_with_port(16551);
    d.run("scene_create", json!({ "path": "tmp/test.tscn", ... }))
        .expect("scene_create failed");
    d.run("scene_instance", json!({ ... }))
        .expect("instance failed");
    d.quit().expect("quit failed");
}
```

### Key behaviors
- Monitors stdout for `{"source":"director","status":"ready"}` before returning
- Spawns background thread to drain stdout (prevents pipe SIGPIPE)
- TCP uses same length-prefixed JSON codec as wire protocol
- `Drop` sends quit then force-kills if needed

---

## Environment Variables

Both harnesses respect:
- `GODOT_BIN` — override Godot binary name (default: `"godot"`)
- `THEATRE_PORT` — set automatically by `GodotFixture::start` via `THEATRE_PORT` env

---

## When to Use

- **GodotFixture**: testing TCP wire protocol, query/response behavior, reconnect scenarios
- **DirectorFixture**: isolated scene-editor operations with no shared state between tests
- **DaemonFixture**: multi-step workflows where maintaining a persistent Godot session matters

## When NOT to Use

- Do not use these in `#[cfg(test)] mod tests` inside library crates — those use **inline-test-fixtures** instead
- All E2E tests must be `#[ignore = "requires Godot binary and built GDExtension"]`

## Common Violations

- Forgetting `#[ignore]` on E2E tests (breaks CI when Godot binary not present)
- Using `DaemonFixture` when operations are independent (prefer `DirectorFixture`)
- Not calling `disconnect_keep_alive` in resilience tests (fixture Drop kills process too early)
