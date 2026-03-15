# Design: Live Journey Tests

## Overview

A new test crate (`tests/live-tests/`) for ad-hoc journey tests that run against
a **windowed** (non-headless) Godot instance with a real GPU. These tests cover
capabilities that headless E2E tests cannot exercise: screenshot rendering,
physics simulation over time, watch/delta with real gameplay state changes, and
cross-tool Director→Stage workflows.

Every test scenario is implemented **twice** via a shared trait: once shelling
out to the CLI binaries (`stage`, `director`), and once using the in-process
MCP server API (like `E2EHarness`). This ensures both interfaces receive equal
coverage.

### What headless tests can't cover

| Gap | Why headless fails | Live test approach |
|-----|--------------------|--------------------|
| Screenshots | `--headless` skips viewport render; `screenshot_at` returns empty | Windowed Godot renders to viewport; verify image data is non-empty JPEG |
| Physics simulation | Headless physics works but scenes are static | New scenes with moving bodies, gravity, collisions — verify position changes over time |
| Watch triggers from gameplay | Existing tests teleport manually; no organic state changes | Enemies patrol and take damage — watches fire from real gameplay |
| Director→Stage cross-tool | Director tests run headless subprocess; Stage tests use separate Godot instance | Single live Godot loads Director-built scenes, then Stage observes them |

### Not in CI

These tests are `#[ignore = "requires display and Godot binary"]` and are run
ad-hoc on developer machines with a GPU and display. They are NOT expected to
pass in headless CI environments.

---

## Implementation Units

### Unit 1: Crate Skeleton

**File**: `tests/live-tests/Cargo.toml`

```toml
[package]
name = "live-tests"
version = "0.0.0"
edition = "2024"
publish = false

# Ad-hoc live journey tests requiring a display and Godot binary.
# Run with: cargo test -p live-tests -- --include-ignored --nocapture
# NOT for CI — requires GPU, display, and windowed Godot.

[lib]
name = "live_tests"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tokio = { version = "1", features = ["full"] }
stage-server = { path = "../../crates/stage-server" }
stage-protocol = { path = "../../crates/stage-protocol" }
director = { path = "../../crates/director" }
rmcp = { version = "0.1" }
```

**File**: `tests/live-tests/src/lib.rs`

```rust
pub mod harness;
pub mod scenes;

// Test modules
mod test_screenshots;
mod test_physics;
mod test_watch_gameplay;
mod test_director_stage;
```

Add to workspace `Cargo.toml`:

```toml
members = [
    # ... existing ...
    "tests/live-tests",
]
```

Do NOT add to `default-members` (same treatment as wire-tests/director-tests).

**Acceptance Criteria**:
- [ ] `cargo build -p live-tests` succeeds
- [ ] Crate is in workspace members but not default-members

---

### Unit 2: Live Godot Process (Windowed)

**File**: `tests/live-tests/src/harness/godot_process.rs`

Reuses the same pattern as `crates/stage-server/tests/support/godot_process.rs`
but launches **without** `--headless`. The key difference: windowed Godot needs
a display and takes longer to start (GPU init, window creation).

```rust
pub struct LiveGodotProcess {
    child: Child,
    port: u16,
    stderr_log: PathBuf,
}

impl LiveGodotProcess {
    /// Launch Godot windowed (no --headless) with the test project.
    ///
    /// Uses --fixed-fps 60 for deterministic physics.
    /// Waits up to 30 seconds for TCP listener (longer than headless due to GPU init).
    /// Captures stderr to temp file for debugging.
    pub async fn start(scene: &str) -> anyhow::Result<Self> { ... }

    pub async fn start_live_3d() -> anyhow::Result<Self> {
        Self::start("res://live_scene_3d.tscn").await
    }

    pub async fn start_live_physics() -> anyhow::Result<Self> {
        Self::start("res://live_scene_physics.tscn").await
    }

    pub fn port(&self) -> u16 { ... }
    pub fn stderr_output(&self) -> String { ... }
}

impl Drop for LiveGodotProcess {
    fn drop(&mut self) {
        // kill + wait
    }
}
```

**Implementation Notes**:
- Command args: `["--fixed-fps", "60", "--path", &project_dir, scene]` — note no `--headless`
- Timeout: 30 seconds (env `LIVE_TIMEOUT_SECS`, default 30) — GPU/window init is slower
- Port allocation: same ephemeral port trick as existing `GodotProcess`
- Environment: sets `THEATRE_PORT` for the addon
- Project dir: resolves to `tests/godot-project/` relative to `CARGO_MANIFEST_DIR`

**Acceptance Criteria**:
- [ ] `LiveGodotProcess::start("res://test_scene_3d.tscn")` opens a visible Godot window
- [ ] TCP listener becomes connectable within timeout
- [ ] Drop kills the process and closes the window

---

### Unit 3: Dual-Interface Test Backend Trait

The core design that enables identical tests for CLI and MCP: a trait that
abstracts tool invocation, with two implementations.

**File**: `tests/live-tests/src/harness/backend.rs`

```rust
use serde_json::Value;

/// Result of a tool invocation — success JSON or error details.
pub enum ToolResult {
    Ok(Value),
    Err { code: String, message: String },
}

impl ToolResult {
    pub fn unwrap_data(self) -> Value { ... }
    pub fn unwrap_err(self) -> (String, String) { ... }
    pub fn is_ok(&self) -> bool { ... }
}

/// Abstraction over CLI subprocess vs in-process MCP server.
///
/// Both Stage and Director tools are dispatched through this trait.
/// Tests are generic over `B: LiveBackend` so they run on both backends.
#[async_trait::async_trait]
pub trait LiveBackend: Send + Sync {
    /// Invoke a Stage tool (spatial_snapshot, spatial_inspect, etc.)
    async fn stage(&self, tool: &str, params: Value) -> anyhow::Result<ToolResult>;

    /// Invoke a Director operation (scene_create, node_add, etc.)
    async fn director(&self, operation: &str, params: Value) -> anyhow::Result<ToolResult>;

    /// Wait for N physics frames at 60 FPS.
    async fn wait_frames(&self, n: u32);

    /// Whether this backend maintains a persistent session (MCP) or is stateless (CLI).
    fn is_stateful(&self) -> bool;
}
```

**File**: `tests/live-tests/src/harness/cli_backend.rs`

```rust
/// CLI backend: shells out to `stage` and `director` binaries per invocation.
pub struct CliBackend {
    godot: LiveGodotProcess,
    project_dir: PathBuf,
}

impl CliBackend {
    pub async fn start(scene: &str) -> anyhow::Result<Self> { ... }
}

#[async_trait::async_trait]
impl LiveBackend for CliBackend {
    async fn stage(&self, tool: &str, params: Value) -> anyhow::Result<ToolResult> {
        // Shell out to `stage <tool> '<json>'` with THEATRE_PORT env
        // Parse stdout JSON, map exit code to ToolResult
    }

    async fn director(&self, operation: &str, params: Value) -> anyhow::Result<ToolResult> {
        // Shell out to `director <operation> '<json>'`
        // Injects project_path into params if not present
    }

    async fn wait_frames(&self, n: u32) {
        let ms = (n as u64 * 1000) / 60 + 50;
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn is_stateful(&self) -> bool { false }
}
```

**File**: `tests/live-tests/src/harness/mcp_backend.rs`

```rust
use stage_server::server::StageServer;
use stage_server::tcp::{SessionState, tcp_client_loop};
use std::sync::Arc;
use tokio::sync::Mutex;

/// MCP backend: in-process StageServer with real TCP connection to Godot.
/// Director operations use subprocess (same as CLI) since Director has no
/// persistent session model.
pub struct McpBackend {
    godot: LiveGodotProcess,
    server: StageServer,
    state: Arc<Mutex<SessionState>>,
    _tcp_task: JoinHandle<()>,
    project_dir: PathBuf,
}

impl McpBackend {
    pub async fn start(scene: &str) -> anyhow::Result<Self> {
        // 1. Start LiveGodotProcess (windowed)
        // 2. Spawn tcp_client_loop connecting to godot.port()
        // 3. Wait for connected
        // 4. Create StageServer::new(state)
    }
}

#[async_trait::async_trait]
impl LiveBackend for McpBackend {
    async fn stage(&self, tool: &str, params: Value) -> anyhow::Result<ToolResult> {
        // Use dispatch_tool() from support module (same as E2EHarness)
        // Maps Result<Value, McpError> to ToolResult
    }

    async fn director(&self, operation: &str, params: Value) -> anyhow::Result<ToolResult> {
        // Same subprocess approach as CliBackend::director()
        // Director is stateless, so no in-process variant needed
    }

    async fn wait_frames(&self, n: u32) {
        let ms = (n as u64 * 1000) / 60 + 50;
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn is_stateful(&self) -> bool { true }
}

impl Drop for McpBackend {
    fn drop(&mut self) {
        self._tcp_task.abort();
    }
}
```

**File**: `tests/live-tests/src/harness/mod.rs`

```rust
mod godot_process;
mod backend;
mod cli_backend;
mod mcp_backend;

pub use backend::{LiveBackend, ToolResult};
pub use cli_backend::CliBackend;
pub use mcp_backend::McpBackend;
pub use godot_process::LiveGodotProcess;

/// Dispatch helper: same as stage-server/tests/support/mod.rs dispatch_tool
/// but re-exported here to avoid cross-crate test dependency.
pub(crate) async fn dispatch_tool(
    server: &StageServer,
    name: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, rmcp::model::ErrorData> { ... }
```

**Implementation Notes**:
- `async_trait` crate is needed for the trait (or use Rust 2024 native async traits
  if the MSRV supports it — check `edition = "2024"` which should support RPITIT).
  With edition 2024, native `async fn` in traits should work without `async_trait`.
  Use `trait LiveBackend: Send + Sync` with `async fn` directly.
- `dispatch_tool` is duplicated from `stage-server/tests/support/mod.rs` because
  test support modules aren't published as library code. Consider extracting to
  a shared crate if this grows, but for now duplication is acceptable.

**Acceptance Criteria**:
- [ ] `CliBackend::start("res://test_scene_3d.tscn")` creates a working backend
- [ ] `McpBackend::start("res://test_scene_3d.tscn")` creates a working backend
- [ ] Both backends can invoke `stage("spatial_snapshot", json!({"detail":"summary"}))` and get valid JSON
- [ ] Both backends can invoke `director("scene_list", json!({"project_path": "..."}))` and get valid JSON

---

### Unit 4: Live Test Scenes (GDScript + .tscn)

New scenes in `tests/godot-project/` with actual gameplay behavior for live testing.

**File**: `tests/godot-project/live_scene_3d.tscn`

A 3D scene with actual movement and gameplay logic:

```
LiveScene3D (Node3D)                    [live_scene_3d.gd]
├── Camera3D                            Positioned for good overview
├── Player (CharacterBody3D)            [live_player.gd] — group "player"
│   ├── CollisionShape3D                CapsuleShape3D
│   └── MeshInstance3D                  Visible capsule mesh
├── Enemies (Node3D)
│   ├── Patrol (CharacterBody3D)        [live_patrol.gd] — group "enemies"
│   │   ├── CollisionShape3D            CapsuleShape3D
│   │   └── MeshInstance3D              Red capsule mesh
│   └── Stationary (CharacterBody3D)    [live_enemy.gd] — group "enemies"
│       ├── CollisionShape3D            CapsuleShape3D
│       └── MeshInstance3D              Blue capsule mesh
├── Props (Node3D)
│   ├── Crate (RigidBody3D)            [live_crate.gd] — group "props"
│   │   ├── CollisionShape3D            BoxShape3D
│   │   └── MeshInstance3D              Box mesh
│   └── Ball (RigidBody3D)             group "props"
│       ├── CollisionShape3D            SphereShape3D
│       └── MeshInstance3D              Sphere mesh
├── Floor (StaticBody3D)
│   ├── CollisionShape3D                BoxShape3D (large)
│   └── MeshInstance3D                  Plane mesh
├── DirectionalLight3D
└── WorldEnvironment                    Default environment for rendering
```

**File**: `tests/godot-project/live_scene_3d.gd`

```gdscript
extends Node3D

func _ready() -> void:
    pass

func ping() -> String:
    return "pong"
```

**File**: `tests/godot-project/live_player.gd`

```gdscript
extends CharacterBody3D

@export var health: int = 100
@export var speed: float = 5.0

## Visible mesh for screenshot verification
```

**File**: `tests/godot-project/live_patrol.gd`

```gdscript
extends CharacterBody3D

@export var health: int = 80
@export var speed: float = 2.0
@export var patrol_range: float = 4.0

var _start_pos: Vector3
var _direction: float = 1.0
var _elapsed: float = 0.0

func _ready() -> void:
    _start_pos = global_position

func _physics_process(delta: float) -> void:
    _elapsed += delta
    # Patrol back and forth along X axis
    var offset = sin(_elapsed * speed) * patrol_range
    global_position = _start_pos + Vector3(offset, 0, 0)
```

Key behavior: the patrol enemy moves sinusoidally along X. Tests can observe
position changes over time via snapshots and verify watch triggers.

**File**: `tests/godot-project/live_enemy.gd`

```gdscript
extends CharacterBody3D

@export var health: int = 60
@export var speed: float = 0.0

func take_damage(amount: int) -> void:
    health -= amount
    if health <= 0:
        health = 0
```

Stationary enemy with a callable `take_damage` method for testing
`spatial_action(call_method)` + watch triggers on health change.

**File**: `tests/godot-project/live_crate.gd`

```gdscript
extends RigidBody3D

@export var weight: float = 10.0
```

RigidBody3D that will fall and settle due to gravity — tests can verify
physics simulation by checking position changes over time.

**File**: `tests/godot-project/live_scene_physics.tscn`

A physics-focused scene for gravity and collision tests:

```
LiveScenePhysics (Node3D)               [live_scene_3d.gd]
├── Camera3D
├── Floor (StaticBody3D)                 at y=0
│   ├── CollisionShape3D                 BoxShape3D (20x0.2x20)
│   └── MeshInstance3D
├── FallingBox (RigidBody3D)             at y=10 — will fall to floor
│   ├── CollisionShape3D                 BoxShape3D
│   └── MeshInstance3D
├── StackA (RigidBody3D)                 at y=1 on floor
│   ├── CollisionShape3D                 BoxShape3D
│   └── MeshInstance3D
├── StackB (RigidBody3D)                 at y=2 on StackA
│   ├── CollisionShape3D                 BoxShape3D
│   └── MeshInstance3D
├── DirectionalLight3D
└── WorldEnvironment
```

FallingBox starts at y=10 and should settle near y≈0.5 (half box height).
StackA/StackB test stacking physics.

**Acceptance Criteria**:
- [ ] `live_scene_3d.tscn` loads without errors in windowed Godot
- [ ] Patrol enemy visibly moves back and forth
- [ ] RigidBody props respond to gravity
- [ ] All meshes render visible geometry (not invisible nodes)
- [ ] `live_scene_physics.tscn` loads; FallingBox falls to floor within 2 seconds

---

### Unit 5: Screenshot & Rendering Tests

**File**: `tests/live-tests/src/test_screenshots.rs`

Tests that verify the clips screenshot system works with a real GPU renderer.
This is the primary capability gap — headless Godot returns empty screenshots.

```rust
/// Macro to generate identical tests for both backends.
macro_rules! live_test {
    ($name:ident, $backend:ty, $scene:expr, $body:expr) => {
        #[tokio::test]
        #[ignore = "requires display and Godot binary"]
        async fn $name() {
            let backend = <$backend>::start($scene).await
                .expect("Failed to start live Godot");
            $body(&backend).await;
        }
    };
}

/// Generates both cli_ and mcp_ prefixed test functions for a given test body.
macro_rules! dual_test {
    ($name:ident, $scene:expr, |$b:ident| $body:block) => {
        mod $name {
            use super::*;

            live_test!(cli, CliBackend, $scene, |$b: &CliBackend| $body);
            live_test!(mcp, McpBackend, $scene, |$b: &McpBackend| $body);
        }
    };
}
```

Test scenarios:

```rust
/// Screenshot capture returns real JPEG image data with non-zero dimensions.
///
/// Steps:
///   1. wait_frames(120) — accumulate ~2 seconds of buffer with rendered frames
///   2. clips(save) → get clip_id with frames > 0
///   3. clips(screenshot_at, clip_id, at_frame=0) → should return 2 content blocks:
///      text metadata + base64 JPEG image
///   4. Verify image data is non-empty and JPEG (starts with /9j in base64)
///   5. clips(delete, clip_id) → cleanup
async fn screenshot_returns_real_image(b: &impl LiveBackend) { ... }

/// Screenshot buffer accumulates frames over time.
///
/// Steps:
///   1. clips(status) → screenshot_buffer_count = 0 or small
///   2. wait_frames(120)
///   3. clips(status) → screenshot_buffer_count > initial count
async fn screenshot_buffer_grows(b: &impl LiveBackend) { ... }

/// Viewport snapshot at multiple time points returns different images
/// (scene has moving entities, so frames should differ).
///
/// Steps:
///   1. wait_frames(60) → 1 second of gameplay
///   2. clips(save) → clip_id_a
///   3. wait_frames(120) → 2 more seconds (patrol has moved)
///   4. clips(save) → clip_id_b
///   5. Verify both clips have screenshots
///   6. Compare frame counts — clip_b should have more frames
async fn different_timepoints_capture_different_data(b: &impl LiveBackend) { ... }
```

**Implementation Notes**:
- The `dual_test!` macro is the core mechanism for test duplication. Each test
  body is written once as an async fn generic over `impl LiveBackend`, then
  instantiated for both `CliBackend` and `McpBackend`.
- For screenshot verification via CLI: `clips screenshot_at` returns raw JSON
  to stdout with base64 image data embedded. Parse the content blocks from the
  JSON response.
- For MCP backend: use `dispatch_tool_result` to get `CallToolResult` with
  image content blocks directly.
- The CLI backend cannot get `CallToolResult` directly — it gets JSON text.
  The CLI `clips` command serializes `CallToolResult` content as JSON with
  `type: "image"` blocks. The test must parse this format.

**Acceptance Criteria**:
- [ ] `screenshot_returns_real_image` passes on both CLI and MCP backends
- [ ] Screenshot data is valid JPEG (non-empty base64, correct mime type)
- [ ] `screenshot_buffer_grows` shows increasing buffer over time
- [ ] Tests fail gracefully with clear message if run without display

---

### Unit 6: Physics Simulation Tests

**File**: `tests/live-tests/src/test_physics.rs`

Tests that verify real physics simulation produces expected position changes.

```rust
/// Gravity pulls a RigidBody down over time.
///
/// Steps:
///   1. spatial_snapshot(standard) → FallingBox at y≈10
///   2. wait_frames(120) → 2 seconds of physics at 60 FPS
///   3. spatial_snapshot(standard) → FallingBox at y < 5 (has fallen significantly)
///   4. wait_frames(120) → 2 more seconds
///   5. spatial_snapshot(standard) → FallingBox at y ≈ 0.5 (resting on floor)
async fn gravity_pulls_rigidbody_down(b: &impl LiveBackend) { ... }

/// Patrol enemy position changes between snapshots.
///
/// Steps:
///   1. spatial_snapshot(standard) → Patrol at some position P1
///   2. wait_frames(60) → 1 second
///   3. spatial_snapshot(standard) → Patrol at position P2 ≠ P1
///   4. Assert: |P2.x - P1.x| > 0.5 (patrol moved measurably)
async fn patrol_enemy_moves_over_time(b: &impl LiveBackend) { ... }

/// Stacked RigidBodies settle under gravity without flying apart.
///
/// Steps:
///   1. spatial_snapshot(standard) → StackA at y≈1, StackB at y≈2
///   2. wait_frames(180) → 3 seconds for settling
///   3. spatial_snapshot(standard) → StackA at y≈0.5, StackB at y≈1.5
///   4. Assert: both are within ±0.5 of expected resting positions
///   5. Assert: StackB.y > StackA.y (B is still on top of A)
async fn stacked_rigidbodies_settle(b: &impl LiveBackend) { ... }

/// Teleporting a RigidBody interrupts its physics trajectory.
///
/// Steps:
///   1. wait_frames(30) → let FallingBox start falling
///   2. spatial_action(teleport, FallingBox, [0, 20, 0]) → move it higher
///   3. wait_frames(30) → let it fall again
///   4. spatial_snapshot → FallingBox.y < 20 (has started falling from new position)
///   5. Assert: y is significantly above floor (hasn't had time to reach bottom)
async fn teleport_interrupts_physics(b: &impl LiveBackend) { ... }
```

**Implementation Notes**:
- Physics tests use `live_scene_physics.tscn` which has RigidBodies starting
  at known heights
- Tolerance on position assertions should be generous (±1.0 unit) since
  physics simulation may have minor variations
- The patrol test uses `live_scene_3d.tscn` (has the patrol enemy)
- For CLI backend: each `stage spatial_snapshot` call is a separate connection,
  so the spatial index is rebuilt each time. This is fine for physics tests
  that only check position values.

**Acceptance Criteria**:
- [ ] `gravity_pulls_rigidbody_down` verifies falling object reaches floor
- [ ] `patrol_enemy_moves_over_time` detects patrol movement in both backends
- [ ] `stacked_rigidbodies_settle` verifies stacking stability
- [ ] All tests pass on both CLI and MCP backends

---

### Unit 7: Watch & Delta Gameplay Tests

**File**: `tests/live-tests/src/test_watch_gameplay.rs`

Tests that verify watches and deltas detect real gameplay state changes.
These tests require the **MCP backend** for stateful session (watches/deltas
need a persistent session). The `dual_test!` macro still generates CLI variants,
but stateful tests skip when `!b.is_stateful()`.

```rust
/// Watch triggers when patrol enemy moves past a threshold.
///
/// MCP only (requires persistent session for watch state).
///
/// Steps:
///   1. spatial_snapshot(standard) → baseline, note Patrol position
///   2. spatial_watch(add, node="Enemies/Patrol", track=["position"]) → watch_id
///   3. wait_frames(120) → patrol has moved
///   4. spatial_delta() → Patrol should appear in "moved" array
///   5. Verify delta_pos is non-zero for Patrol
///   6. spatial_watch(remove, watch_id) → cleanup
async fn watch_detects_patrol_movement(b: &impl LiveBackend) { ... }

/// Watch on health triggers after call_method(take_damage).
///
/// MCP only.
///
/// Steps:
///   1. spatial_snapshot(standard) → baseline, Stationary health=60
///   2. spatial_watch(add, node="Enemies/Stationary", conditions=[{property:"health", op:"changed"}])
///   3. spatial_action(call_method, node="Enemies/Stationary", method="take_damage", args=[25])
///   4. wait_frames(5)
///   5. spatial_delta() → Stationary in state_changed, health 60→35
///   6. spatial_inspect(Enemies/Stationary) → health=35
async fn watch_triggers_on_damage(b: &impl LiveBackend) { ... }

/// Delta detects multiple simultaneous changes: patrol moved + health changed.
///
/// MCP only.
///
/// Steps:
///   1. spatial_snapshot(standard) → baseline
///   2. spatial_action(call_method, Stationary, take_damage, [10])
///   3. wait_frames(90) → patrol has moved, damage applied
///   4. spatial_delta() → both Patrol in "moved" AND Stationary in "state_changed"
async fn delta_captures_concurrent_changes(b: &impl LiveBackend) { ... }

/// Config change affects delta output.
///
/// MCP only.
///
/// Steps:
///   1. spatial_config(state_properties=["health"]) → configure state tracking
///   2. spatial_snapshot(standard) → baseline
///   3. spatial_action(set_property, Stationary, health, 10)
///   4. wait_frames(5)
///   5. spatial_delta() → Stationary in state_changed with health change details
async fn config_state_properties_tracked_in_delta(b: &impl LiveBackend) { ... }
```

**Implementation Notes**:
- Stateful tests check `b.is_stateful()` at the start and return early (with
  a message) for CLI backend. This is better than not generating the test at
  all because `cargo test` output shows the test exists but was skipped.
- Watch condition format: `{"property": "health", "op": "changed"}` — verify
  against actual `SpatialWatchParams` struct before implementation.
- The patrol's sinusoidal movement means position changes continuously, making
  delta detection reliable.

**Acceptance Criteria**:
- [ ] Watch triggers fire from real patrol movement (not manual teleport)
- [ ] `take_damage` method call changes health, detected by watch
- [ ] Delta captures both movement and state changes simultaneously
- [ ] CLI variants gracefully skip stateful tests with informative message

---

### Unit 8: Director→Stage Cross-Tool Tests

**File**: `tests/live-tests/src/test_director_stage.rs`

Tests that build scenes with Director, then observe them with Stage in the
same live Godot instance. This is the most complex test category.

```rust
/// Build a scene with Director, then observe it with Stage.
///
/// Steps:
///   1. director(scene_create, path="tmp/live_test.tscn", root_name="TestRoot", root_type="Node3D")
///   2. director(node_add, scene_path="tmp/live_test.tscn", parent="TestRoot",
///               name="Cube", type="MeshInstance3D")
///   3. director(node_set_properties, scene_path="tmp/live_test.tscn", node_path="TestRoot/Cube",
///               properties={"mesh": "new BoxMesh"})
///   4. Verify scene file exists on disk
///   5. stage(scene_tree, action="find", class="MeshInstance3D") → should find nodes
///      (Note: this queries the running scene tree, not the file — Cube won't be in the
///       running scene unless we instance it. Use scene_read instead to verify file.)
///   6. director(scene_read, path="tmp/live_test.tscn") → verify Cube node present
async fn director_creates_scene_stage_reads(b: &impl LiveBackend) { ... }

/// Director adds a node to the running live scene via daemon, Stage observes it.
///
/// This requires DaemonFixture approach — Director daemon connected to the running
/// Godot instance. Out of scope for initial implementation if too complex.
/// Placeholder for future expansion.

/// Director batch creates a complex scene, verifies integrity via scene_read.
///
/// Steps:
///   1. director(batch, operations=[
///        {op: "scene_create", ...},
///        {op: "node_add", ... (5 nodes)},
///        {op: "node_set_properties", ...},
///      ])
///   2. director(scene_read, path) → verify all 5 nodes present with correct types
///   3. director(scene_diff, path_a=path, path_b=path) → no differences (self-diff)
async fn director_batch_builds_scene(b: &impl LiveBackend) { ... }

/// Director creates animation, verifies via animation_read.
///
/// Steps:
///   1. director(scene_create, ...)
///   2. director(node_add, type="AnimationPlayer")
///   3. director(animation_create, animation_name="walk", length=1.0, loop_mode="linear")
///   4. director(animation_add_track, type="position_3d", node_path=".",
///               keyframes=[{time:0, value:[0,0,0]}, {time:1, value:[5,0,0]}])
///   5. director(animation_read, ...) → verify track and keyframes present
async fn director_animation_roundtrip(b: &impl LiveBackend) { ... }
```

**Implementation Notes**:
- Director operations use subprocess mode (not daemon) since each invocation
  is independent. The `director` method on `LiveBackend` handles this.
- Scene files created by Director go into `tests/godot-project/tmp/` (same as
  existing director tests). Clean up after tests if practical.
- The live Godot instance doesn't automatically load scenes created on disk — they
  need to be instanced or the running scene tree won't reflect them. Stage's
  `scene_tree` queries the running tree, not files on disk. Use `director scene_read`
  to verify file contents.
- For a true Director→Stage integration where Stage observes Director-created nodes
  in the live scene, we'd need the Director daemon connected to the running Godot.
  This is a future enhancement — for now, test the file-based workflow.

**Acceptance Criteria**:
- [ ] Director creates a scene file; Director reads it back correctly
- [ ] Batch operations produce a scene with all expected nodes
- [ ] Animation roundtrip preserves track type and keyframe data
- [ ] Tests work on both CLI and MCP backends

---

### Unit 9: Test Runner Macro & Utilities

**File**: `tests/live-tests/src/harness/macros.rs`

```rust
/// Generate identical test functions for both CLI and MCP backends.
///
/// Usage:
///   dual_test!(test_name, "res://scene.tscn", |b| {
///       let snap = b.stage("spatial_snapshot", json!({"detail": "standard"})).await?
///           .unwrap_data();
///       assert!(!snap["entities"].as_array().unwrap().is_empty());
///   });
///
/// Expands to:
///   mod test_name {
///       #[tokio::test]
///       #[ignore = "requires display and Godot binary"]
///       async fn cli() { ... }
///
///       #[tokio::test]
///       #[ignore = "requires display and Godot binary"]
///       async fn mcp() { ... }
///   }
macro_rules! dual_test { ... }

/// For tests that only work on stateful (MCP) backend.
/// Generates MCP test + CLI test that prints skip message and returns.
macro_rules! stateful_test { ... }
```

**File**: `tests/live-tests/src/harness/assertions.rs`

```rust
/// Assert two positions are approximately equal within tolerance.
pub fn assert_pos_approx(actual: &[f64], expected: &[f64], tolerance: f64, label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}: dimension mismatch");
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert!(
            (a - e).abs() < tolerance,
            "{label}: component {i} — expected ~{e}, got {a} (tolerance {tolerance})"
        );
    }
}

/// Extract position as Vec<f64> from entity JSON.
pub fn extract_position(entity: &Value) -> Vec<f64> {
    entity["global_position"]
        .as_array()
        .expect("entity should have global_position")
        .iter()
        .map(|v| v.as_f64().expect("position component should be f64"))
        .collect()
}

/// Find entity by path substring in snapshot entities array.
pub fn find_entity<'a>(entities: &'a [Value], name: &str) -> &'a Value {
    entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains(name)).unwrap_or(false))
        .unwrap_or_else(|| {
            let paths: Vec<&str> = entities.iter().filter_map(|e| e["path"].as_str()).collect();
            panic!("Entity containing '{name}' not found. Available: {paths:?}");
        })
}
```

**Acceptance Criteria**:
- [ ] `dual_test!` generates two test functions per invocation
- [ ] `stateful_test!` generates MCP test + CLI skip-test
- [ ] Assertion helpers produce clear error messages on failure

---

## Implementation Order

1. **Unit 1**: Crate skeleton + workspace registration
2. **Unit 2**: `LiveGodotProcess` (windowed process management)
3. **Unit 4**: Live test scenes (GDScript + .tscn) — needed before any test can run
4. **Unit 9**: Macros and assertion utilities
5. **Unit 3**: `LiveBackend` trait + `CliBackend` + `McpBackend`
6. **Unit 5**: Screenshot tests (simplest to verify — pass/fail is binary)
7. **Unit 6**: Physics tests
8. **Unit 7**: Watch/delta gameplay tests
9. **Unit 8**: Director→Stage cross-tool tests

Dependencies:
- Units 5-8 all depend on 1, 2, 3, 4, 9
- Units 5-8 are independent of each other and can be implemented in any order
- Unit 4 (scenes) can be built incrementally — start with `live_scene_3d.tscn`,
  add `live_scene_physics.tscn` when Unit 6 is reached

## Testing

### Running Live Tests

```bash
# Deploy GDExtension first
theatre deploy ~/dev/theatre/tests/godot-project

# Run all live tests (requires display + Godot)
cargo test -p live-tests -- --include-ignored --nocapture

# Run specific test module
cargo test -p live-tests -- --include-ignored --nocapture test_screenshots
cargo test -p live-tests -- --include-ignored --nocapture test_physics

# Run only CLI backend variants
cargo test -p live-tests -- --include-ignored --nocapture ::cli

# Run only MCP backend variants
cargo test -p live-tests -- --include-ignored --nocapture ::mcp
```

### Test Structure

```
tests/live-tests/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── harness/
    │   ├── mod.rs
    │   ├── godot_process.rs      (LiveGodotProcess — windowed)
    │   ├── backend.rs            (LiveBackend trait + ToolResult)
    │   ├── cli_backend.rs        (CliBackend — subprocess)
    │   ├── mcp_backend.rs        (McpBackend — in-process server)
    │   ├── macros.rs             (dual_test!, stateful_test!)
    │   └── assertions.rs         (position/entity helpers)
    ├── test_screenshots.rs       (4 tests × 2 backends = 8)
    ├── test_physics.rs           (4 tests × 2 backends = 8)
    ├── test_watch_gameplay.rs    (4 tests × 2 backends = 8, CLI skips stateful)
    └── test_director_stage.rs    (3 tests × 2 backends = 6)
```

Total: ~30 test functions (15 unique scenarios × 2 backends).

### Key Test Cases by Module

| Module | Test | CLI | MCP | Scene |
|--------|------|-----|-----|-------|
| screenshots | screenshot_returns_real_image | yes | yes | live_scene_3d |
| screenshots | screenshot_buffer_grows | yes | yes | live_scene_3d |
| screenshots | different_timepoints_capture_different_data | yes | yes | live_scene_3d |
| physics | gravity_pulls_rigidbody_down | yes | yes | live_scene_physics |
| physics | patrol_enemy_moves_over_time | yes | yes | live_scene_3d |
| physics | stacked_rigidbodies_settle | yes | yes | live_scene_physics |
| physics | teleport_interrupts_physics | yes | yes | live_scene_physics |
| watch | watch_detects_patrol_movement | skip | yes | live_scene_3d |
| watch | watch_triggers_on_damage | skip | yes | live_scene_3d |
| watch | delta_captures_concurrent_changes | skip | yes | live_scene_3d |
| watch | config_state_properties_tracked_in_delta | skip | yes | live_scene_3d |
| director | director_creates_scene_stage_reads | yes | yes | live_scene_3d |
| director | director_batch_builds_scene | yes | yes | live_scene_3d |
| director | director_animation_roundtrip | yes | yes | live_scene_3d |

## Verification Checklist

```bash
# 1. Crate builds
cargo build -p live-tests

# 2. Tests are discovered (all should show as ignored)
cargo test -p live-tests -- --list 2>&1 | grep "test_"

# 3. Run with display (ad-hoc)
cargo test -p live-tests -- --include-ignored --nocapture

# 4. Verify windowed Godot opens and closes for each test
# (visible window should appear and disappear)

# 5. Full workspace still builds
cargo build --workspace

# 6. Existing tests still pass
cargo test --workspace
```
