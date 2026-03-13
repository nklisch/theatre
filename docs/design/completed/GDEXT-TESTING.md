# GDExtension Testing Design

## Problem

The GDExtension crate (`spectator-godot`) has ~2500 lines of code with zero
automated tests that exercise it inside Godot. The existing unit tests in
`recorder.rs` only cover SQLite schema and MessagePack serialization — logic
that doesn't touch Godot APIs.

The INTEGRATION-TESTS.md design covers two layers (TCP mock tests and E2E
tests), but both enter through the MCP server side. Neither tests the
GDExtension in isolation. Bugs in collector traversal, action execution,
variant conversion, TCP message handling, or GDScript adapter wiring can only
be caught by manual testing.

Concrete example: **the editor dock buttons do nothing**. The dock (`dock.gd`,
`@tool`) runs in the editor process. The runtime (`runtime.gd`, not `@tool`)
runs in the game process. `_try_acquire_runtime()` loads the script in the
editor and reads its static `instance` var, which is `null` because `_ready()`
only runs in the game process. Every button handler early-returns. This class
of bug — cross-process wiring failure — is invisible without testing.

## Three Complementary Approaches

| Approach | What it validates | Needs Godot? | Speed |
|----------|------------------|-------------|-------|
| **1. TCP wire tests** | GDExtension TCP server, query handler, collector, actions | Yes (headless) | ~5-10s |
| **2. GDScript test runner** | Addon wiring, class instantiation, signal flow, dock behavior | Yes (headless) | ~3-5s |
| **3. Extract + unit test pure logic** | State machines, dispatch tables, conversion rules | No | <1s |

All three are independent — any can be implemented without the others. Together
they cover the full GDExtension surface.

---

## Approach 1: TCP Wire Tests

### Concept

A standalone Rust test binary connects directly to the GDExtension's TCP
listener in a headless Godot instance. It speaks the spectator protocol —
handshake, queries, actions — and asserts on responses. This tests the
GDExtension's half of the stack without spectator-server in the loop.

```
Rust test binary (tokio TcpStream)
    │
    ├── Connects to GDExtension TCP listener
    ├── Receives Handshake → sends HandshakeAck
    ├── Sends Query { method, params }
    ├── Receives Response { data } or Error { code, message }
    └── Assertions on response data
```

### Why this matters

INTEGRATION-TESTS.md's E2E layer enters at the MCP tool handler level.
Responses pass through the server's spatial reasoning, budgeting, and
formatting before reaching assertions. If a wire test fails, the bug is
definitively in the GDExtension. If an E2E test fails, the bug could be in
either half.

### Crate structure

```
tests/
    wire-tests/
        Cargo.toml          → binary test crate, depends on spectator-protocol
        src/
            main.rs         → test runner entry point
            harness.rs      → GodotProcess + TCP client
            test_handshake.rs
            test_snapshot.rs
            test_inspect.rs
            test_scene_tree.rs
            test_actions.rs
            test_spatial_query.rs
            test_recording.rs
    godot-project/          → shared with INTEGRATION-TESTS.md E2E layer
        project.godot
        test_scene_3d.tscn
        test_scene_3d.gd
        test_scene_2d.tscn
        test_scene_2d.gd
        addons/spectator/   → symlink or copy
```

The wire-tests crate is a `[[test]]` target, not part of the default workspace
build. It depends only on `spectator-protocol` (for codec + message types) and
`tokio`.

```toml
# tests/wire-tests/Cargo.toml
[package]
name = "spectator-wire-tests"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
spectator-protocol = { path = "../../crates/spectator-protocol" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
anyhow = "1"
```

### Test harness

```rust
// tests/wire-tests/src/harness.rs

use spectator_protocol::{codec, handshake::Handshake, messages::Message};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct GodotFixture {
    child: Child,
    port: u16,
    stream: TcpStream,
    pub handshake: Handshake,
}

impl GodotFixture {
    /// Launch Godot headless with the test project, connect, complete handshake.
    pub fn start(scene: &str) -> anyhow::Result<Self> {
        let port = portpicker::pick_unused_port()
            .ok_or_else(|| anyhow::anyhow!("no free port"))?;

        let godot_bin = std::env::var("GODOT_BIN").unwrap_or("godot".into());
        let project_dir = Self::project_dir();

        let child = Command::new(&godot_bin)
            .args([
                "--headless",
                "--fixed-fps", "60",
                "--path", &project_dir.to_string_lossy(),
                scene,
            ])
            .env("THEATRE_PORT", port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Wait for TCP listener
        let stream = Self::wait_for_connection(port, Duration::from_secs(10))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        // Read handshake
        let msg = codec::read_message::<Message>(&stream)?;
        let handshake = match msg {
            Message::Handshake(h) => h,
            other => anyhow::bail!("Expected Handshake, got {:?}", other),
        };

        // Send HandshakeAck
        let ack = Message::HandshakeAck(spectator_protocol::handshake::HandshakeAck {
            session_id: "wire-test-session".into(),
        });
        codec::write_message(&stream, &ack)?;

        Ok(Self { child, port, stream, handshake })
    }

    /// Send a query and wait for the response.
    pub fn query(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<QueryResult> {
        let id = uuid_simple();
        let msg = Message::Query {
            id: id.clone(),
            method: method.into(),
            params,
        };
        codec::write_message(&self.stream, &msg)?;

        let response = codec::read_message::<Message>(&self.stream)?;
        match response {
            Message::Response { id: rid, data } if rid == id => {
                Ok(QueryResult::Ok(data))
            }
            Message::Error { id: rid, code, message } if rid == id => {
                Ok(QueryResult::Err { code, message })
            }
            other => anyhow::bail!("Unexpected response: {:?}", other),
        }
    }

    /// Send an event to the addon.
    pub fn send_event(
        &mut self,
        event: &str,
        data: serde_json::Value,
    ) -> anyhow::Result<()> {
        let msg = Message::Event {
            event: event.into(),
            data,
        };
        codec::write_message(&self.stream, &msg)?;
        Ok(())
    }

    fn wait_for_connection(port: u16, timeout: Duration) -> anyhow::Result<TcpStream> {
        let deadline = Instant::now() + timeout;
        loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => return Ok(stream),
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => anyhow::bail!("Godot not listening on {port}: {e}"),
            }
        }
    }

    fn project_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../godot-project")
            .canonicalize()
            .expect("test godot-project dir must exist")
    }
}

impl Drop for GodotFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub enum QueryResult {
    Ok(serde_json::Value),
    Err { code: String, message: String },
}

impl QueryResult {
    pub fn unwrap_data(self) -> serde_json::Value {
        match self {
            Self::Ok(data) => data,
            Self::Err { code, message } => {
                panic!("Expected Ok, got error: {code}: {message}")
            }
        }
    }

    pub fn unwrap_err(self) -> (String, String) {
        match self {
            Self::Err { code, message } => (code, message),
            Self::Ok(data) => panic!("Expected error, got data: {data}"),
        }
    }
}
```

### Test cases

```rust
// test_handshake.rs

#[test]
fn handshake_reports_3d_scene() {
    let fixture = GodotFixture::start("test_scene_3d.tscn").unwrap();
    assert_eq!(fixture.handshake.dimensions, 3);
    assert!(!fixture.handshake.project_name.is_empty());
    assert!(fixture.handshake.godot_version.starts_with("4."));
    assert_eq!(fixture.handshake.physics_ticks, 60);
}

#[test]
fn handshake_reports_2d_scene() {
    let fixture = GodotFixture::start("test_scene_2d.tscn").unwrap();
    assert_eq!(fixture.handshake.dimensions, 2);
}
```

```rust
// test_snapshot.rs

#[test]
fn snapshot_returns_entities_with_positions() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f.query("get_snapshot_data", serde_json::json!({
        "detail_level": "standard"
    })).unwrap().unwrap_data();

    let entities = data["entities"].as_array().unwrap();
    assert!(entities.len() >= 5, "Expected at least Player, 2 enemies, 2 items");

    // Find player
    let player = entities.iter()
        .find(|e| e["path"].as_str().unwrap().contains("Player"))
        .expect("Player entity not found");

    // Player is at origin
    let pos = &player["position"];
    assert_approx(pos[0].as_f64().unwrap(), 0.0);
    assert_approx(pos[1].as_f64().unwrap(), 0.0);
    assert_approx(pos[2].as_f64().unwrap(), 0.0);
}

#[test]
fn snapshot_includes_groups() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f.query("get_snapshot_data", serde_json::json!({
        "detail_level": "standard"
    })).unwrap().unwrap_data();

    let scout = find_entity(&data, "Scout");
    let groups = scout["groups"].as_array().unwrap();
    assert!(groups.iter().any(|g| g == "enemies"));
}

#[test]
fn snapshot_includes_state_exports() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f.query("get_snapshot_data", serde_json::json!({
        "detail_level": "full"
    })).unwrap().unwrap_data();

    let scout = find_entity(&data, "Scout");
    assert_eq!(scout["state"]["health"], 80);
}

#[test]
fn snapshot_2d_has_2_component_positions() {
    let mut f = GodotFixture::start("test_scene_2d.tscn").unwrap();

    let data = f.query("get_snapshot_data", serde_json::json!({
        "detail_level": "standard"
    })).unwrap().unwrap_data();

    let player = find_entity(&data, "Player");
    let pos = player["position"].as_array().unwrap();
    assert_eq!(pos.len(), 2);
}
```

```rust
// test_actions.rs

#[test]
fn teleport_moves_node_and_returns_previous() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f.query("execute_action", serde_json::json!({
        "action": "teleport",
        "path": "TestScene3D/Enemies/Scout",
        "position": [10.0, 0.0, 0.0]
    })).unwrap().unwrap_data();

    assert_eq!(result["action"], "teleport");
    assert_eq!(result["result"], "ok");

    // Previous position was (5, 0, -3)
    let prev = &result["details"]["previous_position"];
    assert_approx(prev[0].as_f64().unwrap(), 5.0);

    // Verify new position via snapshot
    let snap = f.query("get_snapshot_data", serde_json::json!({
        "detail_level": "standard"
    })).unwrap().unwrap_data();

    let scout = find_entity(&snap, "Scout");
    assert_approx(scout["position"][0].as_f64().unwrap(), 10.0);
}

#[test]
fn set_property_changes_value() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f.query("execute_action", serde_json::json!({
        "action": "set_property",
        "path": "TestScene3D/Enemies/Scout",
        "property": "health",
        "value": 42
    })).unwrap().unwrap_data();

    assert_eq!(result["details"]["previous_value"], 80);
    assert_eq!(result["details"]["new_value"], 42);
}

#[test]
fn call_method_returns_result() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f.query("execute_action", serde_json::json!({
        "action": "call_method",
        "path": "TestScene3D",
        "method": "ping",
        "args": []
    })).unwrap().unwrap_data();

    assert_eq!(result["details"]["return_value"], "pong");
}

#[test]
fn action_on_missing_node_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f.query("execute_action", serde_json::json!({
        "action": "teleport",
        "path": "TestScene3D/DoesNotExist",
        "position": [0.0, 0.0, 0.0]
    })).unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

#[test]
fn pause_and_advance_frames() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Pause
    f.query("execute_action", serde_json::json!({
        "action": "pause", "paused": true
    })).unwrap().unwrap_data();

    // Get current frame
    let info1 = f.query("get_frame_info", serde_json::json!({}))
        .unwrap().unwrap_data();
    let frame1 = info1["frame"].as_u64().unwrap();

    // Advance 5 frames (response is deferred — comes after physics ticks)
    let result = f.query("execute_action", serde_json::json!({
        "action": "advance_frames", "frames": 5
    })).unwrap().unwrap_data();

    assert_eq!(result["action"], "advance_frames");
    let new_frame = result["details"]["new_frame"].as_u64().unwrap();
    assert_eq!(new_frame, frame1 + 5);
}
```

```rust
// test_scene_tree.rs

#[test]
fn scene_tree_returns_children() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f.query("get_scene_tree", serde_json::json!({
        "action": "children",
        "path": "TestScene3D/Enemies"
    })).unwrap().unwrap_data();

    let children = data["children"].as_array().unwrap();
    let names: Vec<&str> = children.iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Scout"));
    assert!(names.contains(&"Tank"));
}
```

```rust
// test_inspect.rs

#[test]
fn inspect_returns_all_categories() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f.query("get_node_inspect", serde_json::json!({
        "path": "TestScene3D/Player"
    })).unwrap().unwrap_data();

    assert!(data.get("transform").is_some());
    assert!(data.get("state").is_some());
    assert!(data.get("children").is_some());
}
```

```rust
// test_spatial_query.rs

#[test]
fn raycast_returns_hit_or_clear() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Need to wait one physics frame for collision shapes to register
    f.query("execute_action", serde_json::json!({
        "action": "pause", "paused": true
    })).unwrap();
    f.query("execute_action", serde_json::json!({
        "action": "advance_frames", "frames": 2
    })).unwrap();

    let data = f.query("spatial_query", serde_json::json!({
        "type": "raycast",
        "from": { "position": [0.0, 1.0, 0.0] },
        "to": { "position": [5.0, 1.0, -3.0] }
    })).unwrap().unwrap_data();

    // Response has hit or clear field
    assert!(data.get("hit").is_some() || data.get("clear").is_some());
}
```

```rust
// test_recording.rs

#[test]
fn recording_start_status_stop_lifecycle() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let start = f.query("recording_start", serde_json::json!({
        "name": "wire_test",
        "storage_path": "/tmp/spectator-wire-test/",
        "capture_interval": 1,
        "max_frames": 100
    })).unwrap().unwrap_data();

    assert!(!start["recording_id"].as_str().unwrap().is_empty());

    let status = f.query("recording_status", serde_json::json!({}))
        .unwrap().unwrap_data();
    assert_eq!(status["active"], true);

    let stop = f.query("recording_stop", serde_json::json!({}))
        .unwrap().unwrap_data();
    assert!(stop["frames_captured"].as_u64().unwrap() >= 0);
}
```

### Running

```bash
# Requires Godot binary in PATH (or GODOT_BIN set)
cargo test -p spectator-wire-tests

# Single test
cargo test -p spectator-wire-tests -- test_handshake
```

### What this catches

- Protocol serialization mismatches between server and addon
- Collector bugs (wrong positions, missing groups, broken traversal)
- Action handler bugs (teleport not moving, set_property not applying)
- Query handler dispatch errors (wrong method routing)
- TCP server handshake and message framing bugs
- Recording handler lifecycle bugs
- Crash-on-query regressions

### What this does NOT catch

- GDScript adapter wiring (runtime.gd, dock.gd, plugin.gd)
- Editor dock behavior
- Signal flow between GDScript and GDExtension
- Plugin enable/disable lifecycle

---

## Approach 2: GDScript Test Runner

### Concept

A GDScript that runs inside Godot headless, exercises the GDExtension classes
through their GDScript-facing API (`#[func]` methods, signals), and the addon
wiring layer (runtime.gd, dock.gd). Exits with code 0 on success, 1 on
failure. Captures exactly the class of bug where "the buttons don't work."

### Architecture

```
godot --headless --script res://tests/test_runner.gd --path tests/godot-project
    │
    ├── Instantiates GDExtension classes directly (SpectatorCollector, etc.)
    ├── Instantiates runtime.gd, verifies wiring
    ├── Simulates dock interactions
    ├── Checks signal connections and emission
    └── Prints TAP-format results, exits 0/1
```

### Test output format

TAP (Test Anything Protocol) — simple, parseable, human-readable:

```
TAP version 13
1..12
ok 1 - extension_classes_exist
ok 2 - collector_instantiates
ok 3 - tcp_server_instantiates
ok 4 - recorder_instantiates
ok 5 - runtime_wires_collector_to_server
ok 6 - runtime_wires_recorder_to_server
ok 7 - tcp_server_starts_listening
ok 8 - recorder_start_stop_lifecycle
ok 9 - recorder_signals_fire
ok 10 - collector_tracks_entities
not ok 11 - dock_acquires_runtime  # dock cannot see game-process runtime
ok 12 - keybinds_toggle_pause
```

### File structure

```
tests/godot-project/
    tests/
        test_runner.gd          → entry point, discovers and runs tests
        test_extension.gd       → GDExtension class tests
        test_runtime_wiring.gd  → runtime.gd integration tests
        test_dock.gd            → dock behavior tests
        test_recorder.gd        → recorder lifecycle tests
        test_signals.gd         → signal emission and connection tests
        test_keybinds.gd        → F8/F9/F10 keybind tests
        assert.gd               → assertion helpers
```

### Test runner

```gdscript
# tests/godot-project/tests/test_runner.gd
extends SceneTree

## Discovers test scripts in res://tests/test_*.gd, runs all methods
## starting with "test_", reports TAP output, exits 0 or 1.

var _test_count := 0
var _pass_count := 0
var _fail_count := 0
var _results: Array[String] = []


func _init() -> void:
    # Wait one frame for _ready calls to complete
    await process_frame
    await process_frame

    _discover_and_run()
    _print_results()

    quit(0 if _fail_count == 0 else 1)


func _discover_and_run() -> void:
    var dir := DirAccess.open("res://tests/")
    if not dir:
        push_error("Cannot open res://tests/")
        quit(1)
        return

    dir.list_dir_begin()
    var file := dir.get_next()
    while file != "":
        if file.begins_with("test_") and file.ends_with(".gd") \
                and file != "test_runner.gd":
            _run_test_script("res://tests/" + file)
        file = dir.get_next()


func _run_test_script(path: String) -> void:
    var script: GDScript = load(path)
    if script == null:
        _record(false, path, "failed to load script")
        return

    var instance = script.new()

    # If the test script has a setup method, call it
    if instance.has_method("setup"):
        instance.setup(root)

    for method in instance.get_method_list():
        var name: String = method["name"]
        if not name.begins_with("test_"):
            continue

        _test_count += 1
        var err: String = ""

        # Call the test method — it returns "" on success or error message
        if instance.has_method(name):
            err = instance.call(name)
            if err == null:
                err = ""

        if err == "":
            _pass_count += 1
            _results.append("ok %d - %s/%s" % [_test_count, _basename(path), name])
        else:
            _fail_count += 1
            _results.append("not ok %d - %s/%s  # %s" % [
                _test_count, _basename(path), name, err
            ])

    # Cleanup
    if instance.has_method("teardown"):
        instance.teardown()
    if instance is Node and instance.is_inside_tree():
        instance.queue_free()


func _print_results() -> void:
    print("TAP version 13")
    print("1..%d" % _test_count)
    for line in _results:
        print(line)
    print("# %d passed, %d failed" % [_pass_count, _fail_count])


static func _basename(path: String) -> String:
    return path.get_file().get_basename()
```

### Assertion helper

```gdscript
# tests/godot-project/tests/assert.gd
class_name Assert

static func eq(actual: Variant, expected: Variant, label: String = "") -> String:
    if actual == expected:
        return ""
    return "expected %s got %s%s" % [expected, actual,
        " (%s)" % label if label else ""]

static func true_(val: bool, label: String = "") -> String:
    if val:
        return ""
    return "expected true%s" % [" (%s)" % label if label else ""]

static func false_(val: bool, label: String = "") -> String:
    if not val:
        return ""
    return "expected false%s" % [" (%s)" % label if label else ""]

static func not_null(val: Variant, label: String = "") -> String:
    if val != null:
        return ""
    return "expected non-null%s" % [" (%s)" % label if label else ""]

static func approx(actual: float, expected: float,
        epsilon: float = 0.01, label: String = "") -> String:
    if absf(actual - expected) < epsilon:
        return ""
    return "expected ~%f got %f%s" % [expected, actual,
        " (%s)" % label if label else ""]
```

### Test: GDExtension classes exist and instantiate

```gdscript
# tests/godot-project/tests/test_extension.gd
extends RefCounted

func test_classes_registered() -> String:
    for cls in ["SpectatorTCPServer", "SpectatorCollector", "SpectatorRecorder"]:
        if not ClassDB.class_exists(cls):
            return "class %s not registered" % cls
    return ""

func test_collector_instantiates() -> String:
    var c := SpectatorCollector.new()
    return Assert.not_null(c, "SpectatorCollector.new()")

func test_tcp_server_instantiates() -> String:
    var s := SpectatorTCPServer.new()
    return Assert.not_null(s, "SpectatorTCPServer.new()")

func test_recorder_instantiates() -> String:
    var r := SpectatorRecorder.new()
    return Assert.not_null(r, "SpectatorRecorder.new()")

func test_tcp_server_starts_and_stops() -> String:
    var s := SpectatorTCPServer.new()
    s.start(0)  # ephemeral port
    var err := Assert.true_(s.get_port() > 0, "port assigned")
    if err: return err
    err = Assert.eq(s.get_connection_status(), "waiting", "status after start")
    if err: return err
    s.stop()
    return Assert.eq(s.get_connection_status(), "stopped", "status after stop")

func test_tcp_server_has_activity_signal() -> String:
    var s := SpectatorTCPServer.new()
    return Assert.true_(s.has_signal("activity_received"),
        "activity_received signal")

func test_recorder_has_signals() -> String:
    var r := SpectatorRecorder.new()
    for sig in ["recording_started", "recording_stopped", "marker_added"]:
        if not r.has_signal(sig):
            return "missing signal: %s" % sig
    return ""

func test_collector_initial_counts() -> String:
    var c := SpectatorCollector.new()
    var err := Assert.eq(c.get_tracked_count(), 0, "tracked count")
    if err: return err
    return Assert.eq(c.get_group_count(), 0, "group count")
```

### Test: runtime.gd wiring

```gdscript
# tests/godot-project/tests/test_runtime_wiring.gd
extends RefCounted

var _root: Window

func setup(root: Window) -> void:
    _root = root

func test_runtime_loads() -> String:
    var script: GDScript = load("res://addons/spectator/runtime.gd")
    return Assert.not_null(script, "runtime.gd loads")

func test_runtime_creates_children() -> String:
    # Instantiate runtime like the autoload would
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)

    # runtime._ready() should create collector, tcp_server, recorder
    await _root.get_tree().process_frame

    var err := Assert.not_null(rt.get("tcp_server"), "tcp_server created")
    if err:
        rt.queue_free()
        return err
    err = Assert.not_null(rt.get("collector"), "collector created")
    if err:
        rt.queue_free()
        return err
    err = Assert.not_null(rt.get("recorder"), "recorder created")
    rt.queue_free()
    return err

func test_runtime_wires_collector_to_server() -> String:
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    # Server should be listening (auto_start defaults to true)
    var server = rt.get("tcp_server")
    var err := ""
    if server:
        err = Assert.eq(server.get_connection_status(), "waiting",
            "server listening after runtime._ready()")
    else:
        err = "tcp_server is null"

    rt.queue_free()
    return err

func test_runtime_static_instance_set() -> String:
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    var script: GDScript = load("res://addons/spectator/runtime.gd")
    var instance = script.get("instance")
    var err := Assert.eq(instance, rt, "static instance points to runtime node")

    rt.queue_free()
    return err

func test_runtime_clears_instance_on_exit() -> String:
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    rt.queue_free()
    await _root.get_tree().process_frame

    var script: GDScript = load("res://addons/spectator/runtime.gd")
    var instance = script.get("instance")
    return Assert.eq(instance, null, "static instance cleared after exit")
```

### Test: dock behavior (catches the broken-buttons bug)

```gdscript
# tests/godot-project/tests/test_dock.gd
extends RefCounted

## Tests the dock's ability to acquire the runtime and interact with it.
## This validates the cross-component wiring that's broken when the dock
## runs in the editor process but the runtime runs in the game process.

var _root: Window

func setup(root: Window) -> void:
    _root = root

func test_dock_instantiates() -> String:
    var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
    if not dock_scene:
        return "dock.tscn failed to load"
    var dock := dock_scene.instantiate()
    return Assert.not_null(dock, "dock instantiates")

func test_dock_finds_runtime_in_same_process() -> String:
    # Start runtime first (simulates the game process)
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    # Now create dock in the same tree (simulates same-process scenario)
    var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
    var dock := dock_scene.instantiate()
    _root.add_child(dock)
    await _root.get_tree().process_frame

    # Trigger the dock's runtime acquisition
    dock.call("_try_acquire_runtime")

    # Check if dock found the server
    var server = dock.get("_tcp_server")
    var err: String
    if server and is_instance_valid(server):
        err = ""  # pass — dock can find runtime when in same process
    else:
        err = "dock failed to acquire tcp_server from runtime"

    dock.queue_free()
    rt.queue_free()
    return err

func test_dock_record_button_calls_recorder() -> String:
    # Wire up runtime
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    # Wire up dock
    var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
    var dock := dock_scene.instantiate()
    _root.add_child(dock)
    await _root.get_tree().process_frame
    dock.call("_try_acquire_runtime")

    var recorder = dock.get("_recorder")
    if not recorder or not is_instance_valid(recorder):
        dock.queue_free()
        rt.queue_free()
        return "dock never acquired recorder — button would do nothing"

    # Simulate pressing record
    dock.call("_on_record_pressed")
    await _root.get_tree().process_frame

    var is_recording: bool = recorder.is_recording()

    # Clean up
    if is_recording:
        recorder.stop_recording()
    dock.queue_free()
    rt.queue_free()

    return Assert.true_(is_recording, "recording started after button press")

func test_dock_stop_button_stops_recording() -> String:
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
    var dock := dock_scene.instantiate()
    _root.add_child(dock)
    await _root.get_tree().process_frame
    dock.call("_try_acquire_runtime")

    var recorder = dock.get("_recorder")
    if not recorder or not is_instance_valid(recorder):
        dock.queue_free()
        rt.queue_free()
        return "dock never acquired recorder"

    dock.call("_on_record_pressed")
    await _root.get_tree().process_frame
    dock.call("_on_stop_pressed")
    await _root.get_tree().process_frame

    var still_recording: bool = recorder.is_recording()
    dock.queue_free()
    rt.queue_free()

    return Assert.false_(still_recording, "recording stopped after stop button")

func test_dock_marker_button_adds_marker() -> String:
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
    var dock := dock_scene.instantiate()
    _root.add_child(dock)
    await _root.get_tree().process_frame
    dock.call("_try_acquire_runtime")

    var recorder = dock.get("_recorder")
    if not recorder or not is_instance_valid(recorder):
        dock.queue_free()
        rt.queue_free()
        return "dock never acquired recorder"

    # Must be recording to add marker
    dock.call("_on_record_pressed")
    await _root.get_tree().process_frame

    var marker_fired := false
    recorder.marker_added.connect(func(_f, _s, _l): marker_fired = true)
    dock.call("_on_marker_pressed")
    await _root.get_tree().process_frame

    if recorder.is_recording():
        recorder.stop_recording()
    dock.queue_free()
    rt.queue_free()

    return Assert.true_(marker_fired, "marker_added signal fired")
```

### Test: signal flow

```gdscript
# tests/godot-project/tests/test_signals.gd
extends RefCounted

var _root: Window

func setup(root: Window) -> void:
    _root = root

func test_recorder_emits_recording_started() -> String:
    var collector := SpectatorCollector.new()
    _root.add_child(collector)

    var recorder := SpectatorRecorder.new()
    recorder.set_collector(collector)
    _root.add_child(recorder)
    await _root.get_tree().process_frame

    var signal_data := {}
    recorder.recording_started.connect(func(id: String, name: String):
        signal_data["id"] = id
        signal_data["name"] = name
    )

    recorder.start_recording("test_signal", "/tmp/spectator-gdtest/", 1, 100)
    await _root.get_tree().process_frame

    var err := Assert.not_null(signal_data.get("id"), "recording_started fired")

    if recorder.is_recording():
        recorder.stop_recording()
    recorder.queue_free()
    collector.queue_free()
    return err

func test_recorder_emits_recording_stopped() -> String:
    var collector := SpectatorCollector.new()
    _root.add_child(collector)

    var recorder := SpectatorRecorder.new()
    recorder.set_collector(collector)
    _root.add_child(recorder)
    await _root.get_tree().process_frame

    recorder.start_recording("test_signal", "/tmp/spectator-gdtest/", 1, 100)
    await _root.get_tree().process_frame

    var stopped := false
    recorder.recording_stopped.connect(func(_id, _frames): stopped = true)
    recorder.stop_recording()
    await _root.get_tree().process_frame

    recorder.queue_free()
    collector.queue_free()
    return Assert.true_(stopped, "recording_stopped fired")
```

### Test: keybinds

```gdscript
# tests/godot-project/tests/test_keybinds.gd
extends RefCounted

var _root: Window

func setup(root: Window) -> void:
    _root = root

func test_f10_toggles_pause() -> String:
    var rt = load("res://addons/spectator/runtime.gd").new()
    _root.add_child(rt)
    await _root.get_tree().process_frame

    var tree := _root.get_tree()
    var was_paused := tree.paused

    # Simulate F10 keypress
    var event := InputEventKey.new()
    event.keycode = KEY_F10
    event.pressed = true
    rt._shortcut_input(event)

    var err := Assert.eq(tree.paused, not was_paused, "pause toggled")

    # Restore
    tree.paused = was_paused
    rt.queue_free()
    return err
```

### Running

```bash
# Run all GDScript tests
godot --headless --script res://tests/test_runner.gd --path tests/godot-project --quit-after 30

# Or via a cargo xtask / shell script wrapper
./scripts/test-gdscript.sh
```

### What this catches

- GDExtension classes fail to register or instantiate
- runtime.gd wiring breaks (collector→server, recorder→server connections)
- Dock can't acquire runtime (the actual broken-buttons bug)
- Signal connections missing or wrong signature
- Recorder lifecycle issues visible from GDScript
- Keybind handlers not responding
- Plugin enable/disable side effects

### What this does NOT catch

- Correctness of collector data (positions, groups, state)
- TCP protocol behavior
- Action execution internals
- Performance regressions

---

## Approach 3: Extract + Unit Test Pure Logic

### Concept

Move logic out of `spectator-godot` that doesn't need Godot APIs into
testable locations. The GDExtension becomes a thin adapter that maps between
Godot types and pure-Rust types. The pure logic gets normal `cargo test`
coverage.

### What can be extracted

#### 3a. TCP connection state machine

The `SpectatorTCPServer` has implicit state management: listening → connected
→ handshaking → ready → advancing frames. The frame-advance logic in
`check_frame_advance()` is a state machine that tracks remaining frames and
pending request IDs. This can be a standalone struct.

**Current location**: `tcp_server.rs` lines 324-382, mixed with Godot calls.

**Extracted to**: `spectator-protocol/src/connection_state.rs`

```rust
// crates/spectator-protocol/src/connection_state.rs

/// Pure state machine for the addon-side TCP connection.
/// No Godot types — just state transitions and message decisions.
#[derive(Debug, Default)]
pub struct ConnectionState {
    pub connected: bool,
    pub handshake_completed: bool,
    advance_remaining: u32,
    advance_request_id: Option<String>,
}

/// What the caller (GDExtension) should do after a state transition.
pub enum ConnectionAction {
    /// No action needed.
    None,
    /// Send this message to the client.
    Send(Message),
    /// Disconnect the client.
    Disconnect,
    /// The advance is complete — re-pause and send this response.
    AdvanceComplete {
        response_id: String,
    },
}

impl ConnectionState {
    pub fn on_client_connected(&mut self) -> ConnectionAction {
        self.connected = true;
        self.handshake_completed = false;
        ConnectionAction::None // caller sends handshake
    }

    pub fn on_handshake_ack(&mut self, session_id: &str) -> ConnectionAction {
        self.handshake_completed = true;
        ConnectionAction::None
    }

    pub fn on_handshake_error(&mut self, message: &str) -> ConnectionAction {
        ConnectionAction::Disconnect
    }

    pub fn on_disconnect(&mut self) -> ConnectionAction {
        self.connected = false;
        self.handshake_completed = false;
        self.advance_remaining = 0;
        self.advance_request_id = None;
        ConnectionAction::None
    }

    /// Begin a frame advance. Returns true if accepted.
    pub fn begin_advance(&mut self, frames: u32, request_id: String) -> bool {
        if self.advance_remaining > 0 {
            return false; // already advancing
        }
        self.advance_remaining = frames;
        self.advance_request_id = Some(request_id);
        true
    }

    /// Called each physics tick. Returns action if advance completed.
    pub fn tick_advance(&mut self) -> ConnectionAction {
        if self.advance_remaining == 0 {
            return ConnectionAction::None;
        }
        self.advance_remaining -= 1;
        if self.advance_remaining == 0 {
            if let Some(id) = self.advance_request_id.take() {
                return ConnectionAction::AdvanceComplete { response_id: id };
            }
        }
        ConnectionAction::None
    }

    pub fn is_advancing(&self) -> bool {
        self.advance_remaining > 0
    }

    pub fn is_ready(&self) -> bool {
        self.connected && self.handshake_completed && !self.is_advancing()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_disconnected() {
        let state = ConnectionState::default();
        assert!(!state.connected);
        assert!(!state.handshake_completed);
        assert!(!state.is_advancing());
        assert!(!state.is_ready());
    }

    #[test]
    fn connection_lifecycle() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        assert!(state.connected);
        assert!(!state.handshake_completed);

        state.on_handshake_ack("session-1");
        assert!(state.handshake_completed);
        assert!(state.is_ready());

        state.on_disconnect();
        assert!(!state.connected);
        assert!(!state.is_ready());
    }

    #[test]
    fn advance_frames_lifecycle() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack("s1");

        assert!(state.begin_advance(3, "req-1".into()));
        assert!(state.is_advancing());
        assert!(!state.is_ready());

        // Can't start another advance while one is in progress
        assert!(!state.begin_advance(5, "req-2".into()));

        // Tick down
        assert!(matches!(state.tick_advance(), ConnectionAction::None));
        assert_eq!(state.advance_remaining, 2);

        assert!(matches!(state.tick_advance(), ConnectionAction::None));
        assert_eq!(state.advance_remaining, 1);

        // Final tick completes
        match state.tick_advance() {
            ConnectionAction::AdvanceComplete { response_id } => {
                assert_eq!(response_id, "req-1");
            }
            other => panic!("Expected AdvanceComplete, got {:?}", other),
        }

        assert!(!state.is_advancing());
        assert!(state.is_ready());
    }

    #[test]
    fn disconnect_during_advance_clears_state() {
        let mut state = ConnectionState::default();
        state.on_client_connected();
        state.on_handshake_ack("s1");
        state.begin_advance(10, "req-1".into());

        state.on_disconnect();
        assert!(!state.is_advancing());
        assert_eq!(state.advance_remaining, 0);
        assert!(state.advance_request_id.is_none());
    }
}
```

#### 3b. Query dispatch table

The `query_handler.rs` match on method names can be expressed as a dispatch
table that's testable without Godot:

```rust
// crates/spectator-protocol/src/query_dispatch.rs

/// Known query methods and their parameter types.
/// Used to validate method names and deserialize params before hitting Godot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMethod {
    GetSnapshotData,
    GetFrameInfo,
    GetNodeInspect,
    GetSceneTree,
    ExecuteAction,
    SpatialQuery,
}

impl QueryMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "get_snapshot_data" => Some(Self::GetSnapshotData),
            "get_frame_info" => Some(Self::GetFrameInfo),
            "get_node_inspect" => Some(Self::GetNodeInspect),
            "get_scene_tree" => Some(Self::GetSceneTree),
            "execute_action" => Some(Self::ExecuteAction),
            "spatial_query" => Some(Self::SpatialQuery),
            _ => None,
        }
    }

    /// Validate that params deserialize correctly for this method.
    /// Returns Ok(()) or a human-readable error.
    pub fn validate_params(&self, params: &serde_json::Value) -> Result<(), String> {
        match self {
            Self::GetSnapshotData => {
                serde_json::from_value::<GetSnapshotDataParams>(params.clone())
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            Self::GetFrameInfo => Ok(()), // no params
            Self::GetNodeInspect => {
                serde_json::from_value::<GetNodeInspectParams>(params.clone())
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            Self::GetSceneTree => {
                serde_json::from_value::<GetSceneTreeParams>(params.clone())
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            Self::ExecuteAction => {
                serde_json::from_value::<ActionRequest>(params.clone())
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            Self::SpatialQuery => {
                serde_json::from_value::<SpatialQueryRequest>(params.clone())
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_methods_resolve() {
        assert_eq!(QueryMethod::from_str("get_snapshot_data"),
            Some(QueryMethod::GetSnapshotData));
        assert_eq!(QueryMethod::from_str("execute_action"),
            Some(QueryMethod::ExecuteAction));
    }

    #[test]
    fn unknown_method_returns_none() {
        assert_eq!(QueryMethod::from_str("bogus"), None);
    }

    #[test]
    fn snapshot_params_validate() {
        let params = serde_json::json!({"detail_level": "standard"});
        assert!(QueryMethod::GetSnapshotData.validate_params(&params).is_ok());
    }

    #[test]
    fn snapshot_params_reject_invalid() {
        let params = serde_json::json!({"detail_level": "nonexistent"});
        assert!(QueryMethod::GetSnapshotData.validate_params(&params).is_err());
    }

    #[test]
    fn action_params_validate_teleport() {
        let params = serde_json::json!({
            "action": "teleport",
            "path": "Player",
            "position": [1.0, 2.0, 3.0]
        });
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_ok());
    }

    #[test]
    fn action_params_reject_unknown_action() {
        let params = serde_json::json!({
            "action": "explode",
            "path": "Player"
        });
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_err());
    }
}
```

#### 3c. JSON↔Variant conversion rules

The `json_to_variant` function in `action_handler.rs` has implicit rules
(2-element numeric array → Vector2, 3-element → Vector3). These rules can be
expressed as a pure mapping that's testable, with the actual Godot Variant
construction as a separate step.

```rust
// crates/spectator-protocol/src/variant_mapping.rs

/// What Godot type a JSON value should map to.
/// Determined purely from JSON structure, no Godot dependency.
#[derive(Debug, Clone, PartialEq)]
pub enum VariantTarget {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vector2(f64, f64),
    Vector3(f64, f64, f64),
    Array(Vec<VariantTarget>),
    Dictionary(Vec<(String, VariantTarget)>),
}

impl VariantTarget {
    /// Determine the target Godot type from a JSON value.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        match value {
            serde_json::Value::Null => Ok(Self::Nil),
            serde_json::Value::Bool(b) => Ok(Self::Bool(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Self::Int(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(Self::Float(f))
                } else {
                    Err(format!("unsupported number: {n}"))
                }
            }
            serde_json::Value::String(s) => Ok(Self::String(s.clone())),
            serde_json::Value::Array(arr) => {
                // 2-element all-numeric → Vector2
                if arr.len() == 2 && arr.iter().all(|v| v.is_number()) {
                    let x = arr[0].as_f64().unwrap_or(0.0);
                    let y = arr[1].as_f64().unwrap_or(0.0);
                    return Ok(Self::Vector2(x, y));
                }
                // 3-element all-numeric → Vector3
                if arr.len() == 3 && arr.iter().all(|v| v.is_number()) {
                    let x = arr[0].as_f64().unwrap_or(0.0);
                    let y = arr[1].as_f64().unwrap_or(0.0);
                    let z = arr[2].as_f64().unwrap_or(0.0);
                    return Ok(Self::Vector3(x, y, z));
                }
                // Generic array
                let items = arr.iter()
                    .map(Self::from_json)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Array(items))
            }
            serde_json::Value::Object(map) => {
                let entries = map.iter()
                    .map(|(k, v)| Self::from_json(v).map(|t| (k.clone(), t)))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Dictionary(entries))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_maps_to_nil() {
        assert_eq!(VariantTarget::from_json(&json!(null)).unwrap(), VariantTarget::Nil);
    }

    #[test]
    fn bool_maps_to_bool() {
        assert_eq!(VariantTarget::from_json(&json!(true)).unwrap(), VariantTarget::Bool(true));
    }

    #[test]
    fn integer_maps_to_int() {
        assert_eq!(VariantTarget::from_json(&json!(42)).unwrap(), VariantTarget::Int(42));
    }

    #[test]
    fn float_maps_to_float() {
        assert_eq!(VariantTarget::from_json(&json!(3.14)).unwrap(), VariantTarget::Float(3.14));
    }

    #[test]
    fn two_element_numeric_array_maps_to_vector2() {
        assert_eq!(
            VariantTarget::from_json(&json!([1.0, 2.0])).unwrap(),
            VariantTarget::Vector2(1.0, 2.0)
        );
    }

    #[test]
    fn three_element_numeric_array_maps_to_vector3() {
        assert_eq!(
            VariantTarget::from_json(&json!([1.0, 2.0, 3.0])).unwrap(),
            VariantTarget::Vector3(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn four_element_array_maps_to_generic_array() {
        let result = VariantTarget::from_json(&json!([1, 2, 3, 4])).unwrap();
        assert!(matches!(result, VariantTarget::Array(_)));
    }

    #[test]
    fn mixed_array_maps_to_generic_array_not_vector() {
        // [1, "two"] — not all numeric, so generic array
        let result = VariantTarget::from_json(&json!([1, "two"])).unwrap();
        assert!(matches!(result, VariantTarget::Array(_)));
    }

    #[test]
    fn empty_array_maps_to_generic_array() {
        let result = VariantTarget::from_json(&json!([])).unwrap();
        assert_eq!(result, VariantTarget::Array(vec![]));
    }

    #[test]
    fn one_element_numeric_array_is_generic() {
        // [5.0] — only 1 element, not Vector2/3
        let result = VariantTarget::from_json(&json!([5.0])).unwrap();
        assert!(matches!(result, VariantTarget::Array(_)));
    }

    #[test]
    fn object_maps_to_dictionary() {
        let result = VariantTarget::from_json(&json!({"hp": 100})).unwrap();
        match result {
            VariantTarget::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].0, "hp");
                assert_eq!(entries[0].1, VariantTarget::Int(100));
            }
            other => panic!("Expected Dictionary, got {:?}", other),
        }
    }

    #[test]
    fn nested_structure() {
        let result = VariantTarget::from_json(&json!({
            "pos": [1.0, 2.0, 3.0],
            "name": "Player",
            "alive": true
        })).unwrap();

        match result {
            VariantTarget::Dictionary(entries) => {
                assert_eq!(entries.len(), 3);
                // pos should be Vector3
                let pos = entries.iter().find(|(k, _)| k == "pos").unwrap();
                assert_eq!(pos.1, VariantTarget::Vector3(1.0, 2.0, 3.0));
            }
            other => panic!("Expected Dictionary, got {:?}", other),
        }
    }
}
```

The GDExtension's `json_to_variant` then becomes a thin match on
`VariantTarget` → `Variant`:

```rust
// In action_handler.rs (simplified after extraction)
fn json_to_variant(value: &serde_json::Value) -> Result<Variant, String> {
    match VariantTarget::from_json(value)? {
        VariantTarget::Nil => Ok(Variant::nil()),
        VariantTarget::Bool(b) => Ok(b.to_variant()),
        VariantTarget::Int(i) => Ok(i.to_variant()),
        VariantTarget::Float(f) => Ok(f.to_variant()),
        VariantTarget::String(s) => Ok(GString::from(s.as_str()).to_variant()),
        VariantTarget::Vector2(x, y) => Ok(Vector2::new(x as f32, y as f32).to_variant()),
        VariantTarget::Vector3(x, y, z) => Ok(Vector3::new(x as f32, y as f32, z as f32).to_variant()),
        VariantTarget::Array(items) => { /* ... */ }
        VariantTarget::Dictionary(entries) => { /* ... */ }
    }
}
```

### What extraction catches (via `cargo test`)

- State machine bugs: double-advance, disconnect-during-advance, etc.
- Dispatch table completeness: new methods not added to router
- Param validation: malformed JSON rejected before reaching Godot
- Variant mapping edge cases: ambiguous arrays, nested structures
- All of the above run in <1s with no Godot dependency

### What extraction does NOT catch

- The thin Godot adapter layer (actual `Variant` construction, scene tree calls)
- Bugs in how extracted types map to Godot runtime behavior
- Any logic that inherently requires Godot APIs (raycasts, scene traversal)

---

## Shared Test Godot Project

All three approaches share `tests/godot-project/`. The project from
INTEGRATION-TESTS.md already defines `test_scene_3d.tscn` and
`test_scene_2d.tscn`. The GDScript tests (Approach 2) add `tests/*.gd` inside
the same project.

### Additions to project structure

```
tests/godot-project/
    project.godot
    addons/spectator/         → symlink to ../../addons/spectator/
    test_scene_3d.tscn
    test_scene_3d.gd
    test_scene_2d.tscn
    test_scene_2d.gd
    tests/                    → GDScript test suite (Approach 2)
        test_runner.gd
        test_extension.gd
        test_runtime_wiring.gd
        test_dock.gd
        test_signals.gd
        test_keybinds.gd
        assert.gd
tests/wire-tests/             → Rust TCP wire test crate (Approach 1)
    Cargo.toml
    src/
        main.rs
        harness.rs
        test_*.rs
```

### THEATRE_PORT env var support

All approaches need the addon to listen on a test-chosen port. Add to
`runtime.gd`:

```gdscript
# In runtime.gd _ready(), before tcp_server.start():
var port: int = 0
var env_port := OS.get_environment("THEATRE_PORT")
if not env_port.is_empty():
    port = env_port.to_int()
if port == 0:
    port = ProjectSettings.get_setting("spectator/connection/port", 9077)
tcp_server.start(port)
```

---

## Implementation Order

1. **Approach 3** (extract pure logic) — no dependencies, immediate value,
   fast CI feedback. Start with `ConnectionState` since the frame-advance
   state machine is the most bug-prone extracted piece.

2. **Approach 2** (GDScript runner) — diagnoses the broken-buttons bug
   immediately. Low effort: ~200 lines of GDScript. Needs Godot binary but
   runs in seconds.

3. **Approach 1** (wire tests) — most thorough GDExtension coverage. Depends
   on the test Godot project existing (shared with INTEGRATION-TESTS.md).

Each approach is independently valuable. Approach 3 runs in normal `cargo test`
CI. Approaches 1 and 2 need Godot and can share a CI job.

---

## CI Integration

```yaml
# Fast tests (no Godot needed) — always run
- name: Unit + extracted logic tests
  run: cargo test --workspace

# GDExtension tests (need Godot) — gated
gdext-tests:
  name: GDExtension Tests
  runs-on: ubuntu-latest
  needs: check
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - uses: chickensoft-games/setup-godot@v2
      with:
        version: 4.6.1

    - name: Build GDExtension
      run: cargo build -p spectator-godot

    - name: Setup test project
      run: ./scripts/setup-test-project.sh

    - name: GDScript tests (Approach 2)
      run: |
        godot --headless --quit-after 30 \
          --script res://tests/test_runner.gd \
          --path tests/godot-project

    - name: Wire tests (Approach 1)
      run: cargo test -p spectator-wire-tests
```
