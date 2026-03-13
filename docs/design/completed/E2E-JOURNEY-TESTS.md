# Design: E2E Journey Tests (Real Godot)

## Overview

End-to-end tests that launch a real Godot binary in headless mode, connect
`spectator-server` to the real addon, and exercise multi-step agent debugging
journeys against a live scene tree. These test the boundaries that mock tests
**cannot**: GDExtension collector accuracy, real physics/transform data, TCP
wire fidelity under actual Godot timing, and the full handshake-to-tool-call
pipeline.

The existing Layer 1 tests (tcp_mock, scenarios) verify server logic in
isolation. These Layer 2 tests verify the **integration contract** — that the
data the real addon sends matches what the server expects, that actions
actually mutate Godot state, and that the full system works as a developer
or agent would use it.

**Few but deep**: 4 journey tests, each 6-12 steps, each telling a real
debugging story.

---

## What These Tests Catch That Mocks Can't

| Bug class | Example | Why mocks miss it |
|-----------|---------|-------------------|
| Collector drift | Collector reports `position` as `[x, z, y]` instead of `[x, y, z]` | Mock fixtures use hardcoded correct data |
| Property mapping | `@export var health: int` serialized as float by Godot variant system | Mock returns `json!(80)` directly |
| Action side effects | `teleport` doesn't update physics state until next frame | Mock teleport is instant |
| Scene tree shape | Real node paths include autoload nodes mock doesn't know about | Mock uses flat entity list |
| Recording frame timing | Recorder captures frames only on `_physics_process` ticks | Mock returns hardcoded frame counts |
| Transform accuracy | Camera transform → forward vector → bearing calculation chain | Mock hardcodes forward vectors |
| Handshake data | Real `scene_dimensions` detection from scene root type | Mock sends hardcoded `3` |

---

## Infrastructure

### Unit 1: GodotProcess — Headless Launcher

**File**: `crates/spectator-server/tests/support/godot_process.rs`

```rust
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tokio::net::TcpStream;
use tokio::time::{Duration, Instant, sleep};

/// Manages a headless Godot process for E2E tests.
///
/// Launches Godot with --headless --fixed-fps 60, sets THEATRE_PORT to
/// an ephemeral port, and waits for the addon's TCP listener to be ready.
pub struct GodotProcess {
    child: Child,
    port: u16,
    stdout_log: PathBuf,
    stderr_log: PathBuf,
}

impl GodotProcess {
    /// Launch Godot headless with the test project and a specific scene.
    ///
    /// Binds to an ephemeral port (OS-assigned via port 0 trick).
    /// Waits up to 15 seconds for the TCP listener to accept connections.
    /// Captures stdout/stderr to temp files for debugging on failure.
    pub async fn start(scene: &str) -> anyhow::Result<Self> { ... }

    /// Launch with the 3D test scene.
    pub async fn start_3d() -> anyhow::Result<Self> {
        Self::start("res://test_scene_3d.tscn").await
    }

    /// Launch with the 2D test scene.
    pub async fn start_2d() -> anyhow::Result<Self> {
        Self::start("res://test_scene_2d.tscn").await
    }

    pub fn port(&self) -> u16 { self.port }

    /// Read captured stderr (Godot's debug output).
    /// Useful for debugging when a test fails.
    pub fn stderr_output(&self) -> String { ... }

    /// Kill the Godot process and return captured output.
    pub fn kill_and_dump(&mut self) -> String { ... }
}

impl Drop for GodotProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
```

**Implementation Notes**:

Ephemeral port allocation: Bind a `TcpListener` to port 0 to get a free port,
immediately close it, pass that port via `THEATRE_PORT` env var. There's a
small TOCTOU window but it's fine for test environments.

The Godot binary path comes from `GODOT_BIN` env var (default: `godot`).
The test project path is `{manifest_dir}/../../tests/godot-project` (relative
to the spectator-server crate).

Wait-for-ready: Poll `TcpStream::connect()` every 100ms up to 15 seconds.
If it times out, dump Godot's stderr for debugging.

Scene is passed as the last positional argument:
```
godot --headless --fixed-fps 60 --path <project_dir> <scene>
```

**Acceptance Criteria**:
- [ ] Godot starts headless and TCP port becomes connectable
- [ ] THEATRE_PORT env var overrides default 9077
- [ ] Drop kills the child process
- [ ] stderr is capturable for debugging

---

### Unit 2: E2EHarness — Real Godot Test Harness

**File**: `crates/spectator-server/tests/support/e2e_harness.rs`

```rust
use super::godot_process::GodotProcess;
use spectator_server::{server::SpectatorServer, tcp::{SessionState, tcp_client_loop}};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use rmcp::model::ErrorData as McpError;

/// Full-stack E2E harness: real Godot + real SpectatorServer.
///
/// Provides the same `call_tool(name, params)` interface as TestHarness,
/// but against a real running Godot scene. Additionally provides a
/// step-based trace log for debugging multi-step journey failures.
pub struct E2EHarness {
    pub godot: GodotProcess,
    pub server: SpectatorServer,
    pub state: Arc<Mutex<SessionState>>,
    _tcp_task: JoinHandle<()>,
    trace: Vec<StepTrace>,
}

struct StepTrace {
    step: usize,
    tool: String,
    params: serde_json::Value,
    result: Result<serde_json::Value, String>,
    elapsed_ms: u64,
}

impl E2EHarness {
    /// Launch Godot with the 3D test scene and connect.
    pub async fn start_3d() -> anyhow::Result<Self> { ... }

    /// Launch Godot with the 2D test scene and connect.
    pub async fn start_2d() -> anyhow::Result<Self> { ... }

    /// Launch Godot with a specific scene, create server, connect, handshake.
    pub async fn start(scene: &str) -> anyhow::Result<Self> { ... }

    /// Call a tool, logging the step for trace output.
    /// Returns the parsed JSON result.
    pub async fn step(
        &mut self,
        n: usize,
        tool: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> { ... }

    /// Call a tool expecting success. Panics with full journey trace on failure.
    pub async fn expect(
        &mut self,
        n: usize,
        tool: &str,
        params: serde_json::Value,
    ) -> serde_json::Value { ... }

    /// Call a tool expecting failure. Panics with trace if it succeeds.
    pub async fn expect_err(
        &mut self,
        n: usize,
        tool: &str,
        params: serde_json::Value,
    ) -> McpError { ... }

    /// Format the full trace for debugging.
    /// On failure, also includes Godot's stderr output.
    pub fn trace_dump(&self) -> String { ... }

    /// Wait for N physics frames to elapse (polls frame counter via snapshot).
    /// Useful for letting actions take effect.
    pub async fn wait_frames(&mut self, n: u32) {
        tokio::time::sleep(Duration::from_millis(
            (n as u64 * 1000) / 60 + 50  // frame time + margin
        )).await;
    }
}

impl Drop for E2EHarness {
    fn drop(&mut self) {
        self._tcp_task.abort();
        // GodotProcess::drop kills godot
    }
}
```

**Implementation Notes**:

`start()` calls `GodotProcess::start(scene)`, creates `SessionState`,
spawns `tcp_client_loop` connecting to `godot.port()`, waits for
`state.connected == true` (same pattern as mock TestHarness), then creates
`SpectatorServer::new(state)`.

The `step()` method wraps `call_tool()` with timing (`Instant::now()`) and
stores the result in `trace`. On assertion failure, `trace_dump()` prints:

```
E2E Journey Trace (test_scene_3d.tscn):
  Step 1: spatial_snapshot({detail:"standard"}) → OK (45ms)
  Step 2: spatial_inspect({node:"Enemies/Scout"}) → OK (12ms)
  Step 3: spatial_action({action:"teleport",...}) → OK (8ms)
  Step 4: spatial_snapshot({detail:"standard"}) → OK (38ms)
  Step 5: spatial_delta({}) → ERR (5ms)  ← FAILED
    Error: internal_error — delta baseline not established

Godot stderr (last 20 lines):
  [Spectator] TCP: client connected from 127.0.0.1:54321
  [Spectator] Handshake: protocol_version=1, dimensions=3
  ...
```

The `wait_frames()` helper sleeps for `(n/60)*1000 + 50` ms. At
`--fixed-fps 60`, each physics frame is ~16.7ms. The 50ms margin accounts
for scheduling jitter. This is simpler and more reliable than polling the
frame counter.

Tool dispatch reuses the same `call_tool` routing as the mock TestHarness
(spatial_snapshot, spatial_inspect, etc. → `server.spatial_snapshot(Parameters(p))`).
Factor this into a shared trait or function in `support/mod.rs` to avoid
duplication.

**Acceptance Criteria**:
- [ ] `start_3d()` connects to real Godot and completes handshake
- [ ] `expect()` returns real scene data from Godot
- [ ] `expect()` panics with full trace + Godot stderr on failure
- [ ] `wait_frames()` allows physics frames to advance

---

### Unit 3: Shared Tool Dispatch

**File**: `crates/spectator-server/tests/support/mod.rs` (modify existing)

```rust
pub mod fixtures;
pub mod harness;
pub mod mock_addon;

#[cfg(feature = "e2e-tests")]
pub mod godot_process;
#[cfg(feature = "e2e-tests")]
pub mod e2e_harness;

/// Shared tool dispatch: routes tool name + JSON params to SpectatorServer
/// handler methods. Used by both TestHarness and E2EHarness.
pub async fn dispatch_tool(
    server: &SpectatorServer,
    name: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, McpError> { ... }
```

**Implementation Notes**:

Extract the match block from `TestHarness::call_tool` into this standalone
function. Both `TestHarness::call_tool` and `E2EHarness::step` call it.

**Acceptance Criteria**:
- [ ] Existing TestHarness tests still pass (refactor, not behavior change)
- [ ] E2EHarness uses the same dispatch function

---

### Unit 4: Journey — Agent Investigates a Scene

**File**: `crates/spectator-server/tests/e2e_journeys.rs`

**Story**: An agent connects to a running game and explores the scene to
understand its structure. This is the most common first interaction — the
agent needs situational awareness before it can help debug anything.

```rust
/// Journey: Agent connects and explores a 3D scene.
///
/// Tests the real handshake, real collector data, real scene tree,
/// and real spatial indexing against actual Godot transforms.
///
/// Steps:
///   1. Verify handshake: session connected, dimensions=3, project="SpectatorTests"
///   2. scene_tree() → real hierarchy (TestScene3D, Camera3D, Player, Enemies/Scout, ...)
///   3. spatial_snapshot(summary) → clustered groups, correct entity count
///   4. spatial_snapshot(standard) → per-entity data with real positions
///        Assert: Player at ~(0,0,0), Scout at ~(5,0,-3), Tank at ~(-4,0,2)
///   5. spatial_inspect(Enemies/Scout) → real transform, class, health=80
///   6. spatial_query(nearest, from Player position, k=2) → two closest entities
///        Assert: results exist and distances are geometrically plausible
#[tokio::test]
async fn journey_explore_scene() { ... }
```

**Implementation Notes**:

This is the foundational E2E test — if this passes, the core data pipeline
(collector → TCP → server → spatial processing) is working.

Position assertions use approximate comparison (`(a - b).abs() < 1.0`)
because real transforms may have small offsets from physics settling,
collision shape positions, etc.

Scene tree assertions check for known node paths but don't assert exact
structure (autoload nodes, editor nodes may vary). Check that `Player`,
`Enemies/Scout`, `Camera3D` exist in the tree.

Step 1 verifies the handshake by reading `SessionState` directly — this
catches protocol version mismatches, incorrect dimension detection, and
GDExtension loading failures that would be invisible to tool calls.

**Acceptance Criteria**:
- [ ] Handshake completes with correct project name and dimensions
- [ ] Scene tree contains expected nodes (Player, Enemies/Scout, etc.)
- [ ] Snapshot returns entities with positions matching the .tscn file
- [ ] Inspect returns real exported vars (health=80)
- [ ] Spatial query returns geometrically correct nearest neighbors

---

### Unit 5: Journey — Agent Debugs a Spatial Bug

**File**: `crates/spectator-server/tests/e2e_journeys.rs`

**Story**: An agent teleports an enemy to a new position, verifies the move
with a snapshot, checks that the delta engine tracks the movement, and then
inspects the enemy at its new location to verify physics state.

This tests the most critical cross-boundary interaction: **actions mutate
real Godot state, and that mutation is visible through all observation tools**.

```rust
/// Journey: Teleport an enemy, verify through snapshot + delta + inspect.
///
/// Steps:
///   1. spatial_snapshot(standard) → baseline: Scout at ~(5, 0, -3)
///   2. spatial_action(teleport, Enemies/Scout, [0, 0, 0]) → ack + previous position
///   3. wait_frames(5) → let physics settle
///   4. spatial_snapshot(standard) → Scout now at ~(0, 0, 0)
///        Assert: position changed from step 1
///   5. spatial_delta() → Scout in "moved" array
///        Assert: moved contains Scout with meaningful displacement
///   6. spatial_inspect(Enemies/Scout) → transform at new position, health still 80
///   7. spatial_action(set_property, Enemies/Scout, health, 25) → ack + old value
///   8. wait_frames(2)
///   9. spatial_inspect(Enemies/Scout) → health now 25
///  10. spatial_delta() → Scout in state_changed (health went 80→25)
#[tokio::test]
async fn journey_debug_spatial_bug() { ... }
```

**Implementation Notes**:

`wait_frames(5)` between teleport and snapshot is critical. In real Godot,
`teleport_node` sets `global_position` but the physics server may not update
collision state until the next `_physics_process`. The `--fixed-fps 60` flag
ensures deterministic timing.

The delta between steps 1 and 4 should show Scout as "moved" with a distance
of roughly `sqrt(5² + 3²) ≈ 5.83` units. Assert `> 4.0` to be safe.

Step 7 tests `set_property` against a real exported var. The GDExtension must
correctly map the `health` string to the Godot property path and set it via
`Object::set()`. Step 9 verifies the value persisted.

Step 10's delta should detect `health` changing from 80 to 25 in
`state_changed`. This crosses: MCP handler → TCP → GDExtension collector
(which must re-read the property) → TCP → server delta engine.

**Acceptance Criteria**:
- [ ] Teleport moves Scout to new position in real Godot
- [ ] Post-teleport snapshot shows new position (within tolerance)
- [ ] Delta detects movement between pre/post teleport snapshots
- [ ] set_property changes real Godot exported var
- [ ] Post-set_property inspect shows new value
- [ ] Delta detects state change in exported var

---

### Unit 6: Journey — Recording During Live Session

**File**: `crates/spectator-server/tests/e2e_journeys.rs`

**Story**: An agent starts a recording, takes snapshots while the game runs,
adds a marker, stops the recording, then verifies the recording metadata.
This tests the recorder GDExtension class, the TCP recording protocol, and
session state coherence between recording and spatial tools.

```rust
/// Journey: Record game state, verify recording lifecycle.
///
/// Steps:
///   1. spatial_snapshot(standard) → baseline, note frame number
///   2. recording(start) → recording_id returned
///   3. recording(status) → active=true, recording_id matches
///   4. wait_frames(30) → let recorder capture ~30 frames
///   5. spatial_snapshot(standard) → mid-recording snapshot still works
///        Assert: frame number advanced from step 1
///   6. recording(add_marker, source="agent", label="mid_test") → ack
///   7. wait_frames(30) → more frames
///   8. recording(stop) → frames_captured > 0
///   9. recording(status) → active=false
///  10. spatial_snapshot(standard) → post-recording snapshot still works
///        Assert: session state not corrupted by recording lifecycle
#[tokio::test]
async fn journey_recording_lifecycle() { ... }
```

**Implementation Notes**:

The GDExtension `SpectatorRecorder` captures frames in `_physics_process`.
With `--fixed-fps 60`, waiting 30 frames ≈ 500ms. After stop, the
`frames_captured` count should be between 20 and 60 (timing is approximate
in headless mode).

Step 5 tests that spatial tools work concurrently with an active recording.
The collector serves both the recorder (frame capture) and the TCP server
(query responses) — this tests their coexistence on the main thread.

Step 10 verifies that stopping a recording doesn't corrupt the session state.
This catches a real class of bug: if the recorder's stop handler holds a
mutex while the TCP server tries to respond to a query, the session deadlocks.

Frame number assertions: step 1 captures `frame_0`, step 5 captures
`frame_mid`. Assert `frame_mid > frame_0` (frames are advancing).

**Acceptance Criteria**:
- [ ] Recording start returns a non-empty recording_id
- [ ] Recording status shows active=true with matching id
- [ ] Spatial snapshot works during active recording
- [ ] Frame counter advances between snapshots
- [ ] Marker add succeeds during recording
- [ ] Recording stop reports frames_captured > 0
- [ ] Post-recording snapshot works (session not corrupted)

---

### Unit 7: Journey — 2D Scene Verification

**File**: `crates/spectator-server/tests/e2e_journeys.rs`

**Story**: An agent connects to a 2D scene and verifies that position format,
bearing system, and spatial indexing all adapt correctly. This catches
dimension-detection bugs that only manifest with a real 2D scene root.

```rust
/// Journey: 2D scene returns correct position format and bearings.
///
/// Steps:
///   1. Verify handshake: dimensions=2
///   2. spatial_snapshot(standard) → entities have [x, y] positions (2 elements)
///        Assert: Player at ~(0, 0), Scout2D at ~(200, 100)
///   3. spatial_query(nearest, from [0,0], k=2) → nearest entities
///        Assert: results have 2-element positions
///   4. spatial_inspect(Player) → 2D transform (no z component)
///   5. spatial_action(teleport, Player, [100, 50]) → ack
///   6. wait_frames(3)
///   7. spatial_snapshot(standard) → Player now at ~(100, 50)
#[tokio::test]
async fn journey_2d_scene() { ... }
```

**Implementation Notes**:

Uses `E2EHarness::start_2d()`. The handshake `scene_dimensions` should be
`2` based on the scene root being `Node2D`.

Position arrays in 2D must have exactly 2 elements. The server's bearing
calculation must use 2D math (no elevation). If the collector incorrectly
sends `[x, y, 0]` for a 2D scene, this test catches it because the bearing
would include an elevation field that shouldn't exist.

Teleport in 2D sends a 2-element position array. The GDExtension must handle
this correctly (set `global_position` as `Vector2`, not `Vector3`).

**Acceptance Criteria**:
- [ ] Handshake reports dimensions=2
- [ ] Snapshot positions are 2-element arrays
- [ ] Spatial query works with 2D spatial index
- [ ] Teleport works with 2-element position
- [ ] No 3D-specific fields (elevation) in 2D responses

---

## Cargo.toml Changes

**File**: `crates/spectator-server/Cargo.toml`

```toml
[features]
integration-tests = []
e2e-tests = []           # ← NEW

[[test]]
name = "e2e_journeys"
path = "tests/e2e_journeys.rs"
required-features = ["e2e-tests"]
```

No new dependencies needed — `tokio`, `serde_json`, `anyhow` are already
dev-dependencies or workspace deps.

---

## Test Scene Changes

### 3D Scene Groups

The existing `test_scene_3d.tscn` has `enemies` group on Scout and Tank but
no groups on Player, Items, or Floor. Add groups to make group-filtering
testable:

**File**: `tests/godot-project/test_scene_3d.tscn` (modify)

Add group `player` to Player node.
Add group `items` to HealthPack and Ammo nodes.

### 2D Scene Groups

**File**: `tests/godot-project/test_scene_2d.tscn` (modify)

Add group `player` to Player node.
Add group `enemies` to Scout2D node.

---

## Implementation Order

1. **GodotProcess** — headless launcher with ephemeral port + stderr capture
2. **Shared dispatch** — extract tool routing from TestHarness
3. **E2EHarness** — wires GodotProcess + SpectatorServer + trace logging
4. **Scene groups** — add missing groups to test scenes
5. **Cargo.toml** — add `e2e-tests` feature + `[[test]]` entry
6. **Journey tests** — all 4 journeys in `e2e_journeys.rs`

Steps 1-3 are infrastructure (can be tested with a smoke test).
Step 4 is a trivial .tscn edit.
Step 6 depends on all prior steps.

---

## Running the Tests

```bash
# Prerequisites: Godot 4.x on PATH (or set GODOT_BIN), GDExtension built
theatre-deploy ~/dev/spectator/tests/godot-project  # or copy .so manually

# Run E2E journey tests
cargo test -p spectator-server --features e2e-tests -- --nocapture

# Run specific journey
cargo test -p spectator-server --features e2e-tests journey_explore_scene -- --nocapture

# Run all tests (unit + mock integration + E2E)
cargo test -p spectator-server --features integration-tests,e2e-tests
```

### Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `GODOT_BIN` | Path to Godot binary | `godot` |
| `E2E_TIMEOUT_SECS` | Max seconds to wait for Godot startup | `15` |

---

## Debugging Failures

When a journey test fails, the trace dump provides:

1. **Step-by-step log**: Every tool call with params, result (OK/ERR), and
   timing. Shows exactly which step failed and what the server saw.

2. **Godot stderr**: The addon's `tracing` output including TCP connection
   events, collector activity, and any GDScript errors.

3. **Assertion context**: Each assert includes the full JSON result so you
   can see the actual data, not just "assertion failed."

Example failure output:

```
---- journey_debug_spatial_bug stdout ----
E2E Journey Trace (test_scene_3d.tscn, port 54321):
  Step 1: spatial_snapshot({detail:"standard"}) → OK (52ms)
    entities: 7, frame: 142
  Step 2: spatial_action({action:"teleport",node:"Enemies/Scout",position:[0,0,0]}) → OK (11ms)
  Step 3: [wait 5 frames, 133ms]
  Step 4: spatial_snapshot({detail:"standard"}) → OK (48ms)
    entities: 7, frame: 147
  Step 5: spatial_delta({}) → OK (6ms)
    moved: 1, state_changed: 0

thread 'journey_debug_spatial_bug' panicked at 'Step 5: expected Scout in moved
array but found: [{"path":"Player","displacement":0.003}]

Full result: {"moved":[...],"state_changed":[],"entered":[],"exited":[]}
'
```

## Verification Checklist

```bash
# Build (ensures e2e support code compiles)
cargo build -p spectator-server --features e2e-tests --tests

# Lint
cargo clippy -p spectator-server --features e2e-tests --tests

# Run (requires Godot)
cargo test -p spectator-server --features e2e-tests -- --nocapture

# Existing tests still pass
cargo test -p spectator-server --features integration-tests
```
