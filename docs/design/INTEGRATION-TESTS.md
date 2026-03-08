# Integration Test Suite Design

## Problem

Spectator has unit tests for protocol serialization, codec framing, bearing
math, and spatial indexing — but **zero tests that verify the system works
end-to-end**. A regression in the GDExtension collector, a protocol mismatch
between server and addon, or a broken MCP tool handler can only be caught
manually by running Godot and eyeballing results.

We need automated integration tests that exercise the real data path:

```
MCP tool call → spectator-server → TCP → spectator-godot → Godot scene tree → response → assertions
```

## Design Overview

Two test layers, each valuable independently:

| Layer | What it tests | Requires Godot? | Speed | CI-friendly? |
|-------|--------------|-----------------|-------|--------------|
| **TCP mock tests** | Server handlers, protocol correctness, spatial processing | No | Fast (~1s) | Yes |
| **E2E tests** | Full stack against real Godot binary + real scene data | Yes | Slow (~10-30s) | Optional (needs Godot in CI) |

Both layers live in a new `tests/` crate or in `crates/spectator-server/tests/`.

---

## Layer 1: TCP Mock Tests

### Concept

Spin up a mock TCP listener that speaks the spectator protocol (handshake +
query/response). Create a `SpectatorServer` instance pointing at the mock.
Call MCP tool handlers directly and assert on results.

This tests the **server half** of the stack in isolation — tool parameter
parsing, spatial reasoning (bearings, budgets, deltas, watches), protocol
serialization, and error handling — without needing Godot at all.

### Architecture

```
Test harness
    │
    ├─ Mock TCP Addon (tokio TcpListener on ephemeral port)
    │   ├─ Sends Handshake on connect
    │   ├─ Receives Query messages
    │   ├─ Responds with canned/computed Response messages
    │   └─ Can inject Event messages (signal_emitted)
    │
    ├─ SpectatorServer (real code, real SessionState)
    │   ├─ tcp_client_loop connects to mock addon
    │   └─ Tool handlers called directly via Rust
    │
    └─ Assertions on tool return values (JSON strings)
```

### Mock Addon Design

```rust
// tests/support/mock_addon.rs

/// A programmable mock that acts as the Godot addon's TCP server.
/// Listens on an ephemeral port, completes the handshake, and
/// dispatches query responses from a handler function.
pub struct MockAddon {
    port: u16,
    shutdown_tx: oneshot::Sender<()>,
    join_handle: JoinHandle<()>,
}

/// Handler function type: receives (method, params) → returns response data.
/// Return Err to send an error response.
type QueryHandler = Box<dyn Fn(&str, &Value) -> Result<Value, (String, String)> + Send + Sync>;

impl MockAddon {
    /// Start a mock addon on an ephemeral port.
    /// The handler is called for each query the server sends.
    pub async fn start(handler: QueryHandler) -> Self { ... }

    /// Start with a specific handshake (e.g., different dimensions).
    pub async fn start_with_handshake(
        handshake: Handshake,
        handler: QueryHandler,
    ) -> Self { ... }

    pub fn port(&self) -> u16 { ... }

    /// Push an event to the connected server (e.g., signal_emitted).
    pub async fn push_event(&self, event: &str, data: Value) { ... }

    /// Shut down cleanly.
    pub async fn shutdown(self) { ... }
}
```

### Mock Scene Data

Canonical test scene with known positions. The mock returns this data for
`get_snapshot_data` queries:

```rust
// tests/support/fixtures.rs

/// A deterministic 3D scene with known positions, groups, and properties.
pub fn mock_scene_3d() -> SnapshotResponse {
    SnapshotResponse {
        frame: 100,
        timestamp_ms: 1667,
        perspective: PerspectiveData {
            position: vec![0.0, 5.0, 10.0],
            rotation_deg: vec![0.0, 0.0, 0.0],
            forward: vec![0.0, 0.0, -1.0],
        },
        entities: vec![
            // Player at origin — in group "player"
            entity("Player", "CharacterBody3D", [0.0, 0.0, 0.0], &["player"]),
            // Enemy 5m north — in group "enemies", health=80
            entity_with_state("enemies/scout", "CharacterBody3D",
                [0.0, 0.0, -5.0], &["enemies"], &[("health", json!(80))]),
            // Wall 3m east — static
            entity("walls/east_wall", "StaticBody3D", [3.0, 0.0, 0.0], &["walls"]),
            // Coin 2m south — in group "collectibles"
            entity("items/coin_01", "Area3D", [0.0, 0.0, 2.0], &["collectibles"]),
            // Camera (perspective source)
            entity("Camera3D", "Camera3D", [0.0, 5.0, 10.0], &[]),
        ],
    }
}

/// A deterministic 2D scene.
pub fn mock_scene_2d() -> SnapshotResponse { ... }
```

### Test Cases — TCP Mock Layer

#### Handshake
- `test_handshake_connects_and_gets_session_id` — server connects to mock,
  completes handshake, `SessionState.connected == true`
- `test_handshake_version_mismatch` — mock sends wrong protocol version,
  server sends `HandshakeError` and disconnects
- `test_reconnect_on_disconnect` — mock drops connection, server reconnects
  within retry interval

#### spatial_snapshot
- `test_snapshot_summary_returns_clusters` — summary detail returns clustered
  groups with correct counts
- `test_snapshot_standard_returns_per_entity` — per-entity data with bearings,
  distances, groups
- `test_snapshot_full_includes_physics_and_children` — full detail includes
  transform, physics, script, children
- `test_snapshot_filters_by_group` — `groups: ["enemies"]` returns only enemies
- `test_snapshot_filters_by_radius` — entities beyond radius excluded
- `test_snapshot_offscreen_excluded_by_default` — offscreen entities excluded
  unless `include_offscreen: true`
- `test_snapshot_pagination` — large scene paginated within token budget
- `test_snapshot_expand_cluster` — expand a cluster from summary
- `test_snapshot_perspective_node` — perspective from a specific node
- `test_snapshot_perspective_point` — perspective from world coordinates
- `test_snapshot_budget_block_present` — every response has a `budget` block
- `test_snapshot_2d_bearings` — 2D scene uses 2D bearing system (no elevation)

#### spatial_inspect
- `test_inspect_all_categories` — returns transform, physics, state, children,
  signals, script, spatial_context
- `test_inspect_selective_include` — only requested categories returned
- `test_inspect_node_not_found` — returns `invalid_params` error
- `test_inspect_spatial_context` — nearby entities, areas, camera visibility

#### scene_tree
- `test_scene_tree_roots` — returns top-level nodes
- `test_scene_tree_children` — returns immediate children of a node
- `test_scene_tree_subtree` — recursive tree with depth limit
- `test_scene_tree_ancestors` — parent chain to root
- `test_scene_tree_find_by_class` — find all CharacterBody3D nodes
- `test_scene_tree_find_by_group` — find all nodes in "enemies" group

#### spatial_action
- `test_action_pause` — pause/unpause, verify response
- `test_action_teleport` — move node, verify response has previous position
- `test_action_set_property` — change property, verify old/new values
- `test_action_call_method` — call method, verify result
- `test_action_return_delta` — action with `return_delta: true` includes delta
- `test_action_node_not_found` — error for missing node

#### spatial_query
- `test_query_nearest` — K nearest nodes from spatial index
- `test_query_radius` — all nodes within radius
- `test_query_raycast` — line-of-sight check (delegated to mock)
- `test_query_relationship` — mutual relationship between two nodes
- `test_query_path_distance` — navmesh distance (delegated to mock)

#### spatial_delta
- `test_delta_detects_moved` — entity moves, delta shows `moved`
- `test_delta_detects_state_change` — property changes, delta shows
  `state_changed`
- `test_delta_detects_entered_exited` — node added/removed between snapshots
- `test_delta_signal_emitted` — pushed signal event appears in delta

#### spatial_watch
- `test_watch_add_list_remove` — lifecycle of a watch subscription
- `test_watch_condition_trigger` — health < 20 triggers in delta
- `test_watch_group` — watch `group:enemies` matches all enemies
- `test_watch_clear` — clear removes all watches

#### spatial_config
- `test_config_read_defaults` — no params returns current config
- `test_config_set_static_patterns` — set static patterns, verify in state
- `test_config_set_token_cap` — set token_hard_cap, subsequent queries respect it

#### recording
- `test_recording_start_stop_status` — lifecycle
- `test_recording_list_delete` — list recordings, delete one
- `test_recording_add_marker` — add an agent marker

#### Error handling
- `test_not_connected_error` — tool call when no addon connected
- `test_addon_timeout` — mock doesn't respond within 5s
- `test_addon_error_response` — mock returns error, mapped to McpError

### Helper: Direct Tool Invocation

Instead of going through MCP stdio transport, call tool handlers directly:

```rust
// tests/support/harness.rs

use spectator_server::server::SpectatorServer;
use spectator_server::tcp::SessionState;

pub struct TestHarness {
    pub server: SpectatorServer,
    pub mock: MockAddon,
}

impl TestHarness {
    /// Create server + mock addon, wait for handshake.
    pub async fn new(handler: QueryHandler) -> Self { ... }

    /// Call a tool by name with JSON params, return the result string.
    pub async fn call_tool(&self, name: &str, params: Value) -> Result<String, McpError> {
        // Use the tool_router to dispatch directly
        ...
    }

    /// Parse the JSON result and extract a field.
    pub fn parse_result(result: &str) -> Value {
        serde_json::from_str(result).unwrap()
    }
}
```

---

## Layer 2: E2E Tests (Real Godot Binary)

### Concept

Launch Godot 4.6 in headless mode with a purpose-built test project.
Start spectator-server's TCP client (not the MCP stdio server — we call
handlers directly). Make real tool calls against a real running game scene
with real physics.

### Architecture

```
Test harness
    │
    ├─ Godot (headless, --fixed-fps 60)
    │   ├─ Test project with spectator addon enabled
    │   ├─ Known scene: fixed positions, groups, properties
    │   ├─ SpectatorTCPServer listening on port (ephemeral via env)
    │   └─ SpectatorCollector collecting real scene data
    │
    ├─ SpectatorServer (real TCP client connects to real Godot)
    │   ├─ Handshake with real addon
    │   └─ Tool handlers return real scene data
    │
    └─ Assertions on real Godot scene state
```

### Test Project: `tests/godot-project/`

A minimal Godot project inside the spectator repo, committed to version
control. Not the cosmic showcase — a purpose-built deterministic scene.

#### Project structure

```
tests/godot-project/
    project.godot
    addons/spectator/     → symlink to ../../addons/spectator/
    test_scene_3d.tscn    → deterministic 3D scene
    test_scene_3d.gd      → minimal script with known properties
    test_scene_2d.tscn    → deterministic 2D scene
    test_scene_2d.gd      → minimal script
```

#### 3D Test Scene (`test_scene_3d.tscn`)

A static, deterministic scene with no timers, animations, or randomness:

```
TestScene3D (Node3D) [script: test_scene_3d.gd]
├── Camera3D          @ position (0, 5, 10), looking at origin
├── Player (CharacterBody3D)
│   ├── CollisionShape3D (capsule)
│   └── MeshInstance3D
│   groups: ["player"]
│   @export health: int = 100
│   @export speed: float = 5.0
├── Enemies (Node3D)
│   ├── Scout (CharacterBody3D)  @ position (5, 0, -3)
│   │   ├── CollisionShape3D
│   │   └── MeshInstance3D
│   │   groups: ["enemies"]
│   │   @export health: int = 80
│   │   @export patrol_speed: float = 2.0
│   └── Tank (CharacterBody3D)   @ position (-4, 0, -8)
│       ├── CollisionShape3D
│       └── MeshInstance3D
│       groups: ["enemies"]
│       @export health: int = 200
│       @export patrol_speed: float = 0.5
├── Walls (Node3D)
│   ├── NorthWall (StaticBody3D) @ position (0, 1, -15)
│   └── EastWall (StaticBody3D)  @ position (15, 1, 0)
│   groups: ["walls"]
├── Items (Node3D)
│   ├── HealthPack (Area3D)      @ position (3, 0.5, 2)
│   │   groups: ["collectibles", "health_items"]
│   └── Coin (Area3D)            @ position (-2, 0.5, 4)
│       groups: ["collectibles"]
├── DirectionalLight3D
└── NavigationRegion3D
    └── NavigationMesh
```

#### Test scene script (`test_scene_3d.gd`)

```gdscript
extends Node3D

## Minimal test scene — all state is deterministic and observable.
## No timers, no animations, no randomness.

@export var scene_label: String = "integration_test_3d"

func _ready() -> void:
    pass  # scene is purely static unless manipulated via spatial_action

## Callable method for call_method action tests
func ping() -> String:
    return "pong"

## Method that modifies state for testing
func damage_player(amount: int) -> int:
    var player := $Player
    var old_health: int = player.get("health")
    player.set("health", old_health - amount)
    return player.get("health")
```

#### 2D Test Scene (`test_scene_2d.tscn`)

```
TestScene2D (Node2D)
├── Camera2D           @ position (0, 0)
├── Player (CharacterBody2D)  @ position (100, 300)
│   groups: ["player"]
│   @export health: int = 100
├── Enemy (CharacterBody2D)   @ position (400, 300)
│   groups: ["enemies"]
│   @export health: int = 50
└── Platform (StaticBody2D)   @ position (250, 400)
    groups: ["terrain"]
```

### Godot Lifecycle Management

```rust
// tests/support/godot_process.rs

pub struct GodotProcess {
    child: Child,
    port: u16,
    project_dir: PathBuf,
}

impl GodotProcess {
    /// Launch Godot headless with the test project.
    ///
    /// Uses an ephemeral port to avoid conflicts with parallel tests.
    /// Sets SPECTATOR_PORT env var so the addon listens on the right port.
    /// Uses --fixed-fps 60 for deterministic physics.
    /// Waits for the addon's TCP listener to be ready before returning.
    pub async fn start(scene: &str) -> Result<Self> {
        let port = get_ephemeral_port();

        let child = Command::new("godot")
            .args([
                "--headless",
                "--fixed-fps", "60",
                "--path", &project_dir.to_string_lossy(),
                scene,
            ])
            .env("SPECTATOR_PORT", port.to_string())
            // Godot uses stdout for some logging; capture it
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Wait for TCP listener to be ready (poll connect attempts)
        wait_for_port(port, Duration::from_secs(10)).await?;

        Ok(Self { child, port, project_dir })
    }

    pub fn port(&self) -> u16 { self.port }

    /// Kill the Godot process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for GodotProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Wait until a TCP connection to the port succeeds.
async fn wait_for_port(port: u16, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return Ok(());
        }
        if Instant::now() > deadline {
            anyhow::bail!("Godot addon did not start listening on port {port} within {timeout:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

### E2E Test Harness

```rust
// tests/support/e2e_harness.rs

pub struct E2EHarness {
    pub godot: GodotProcess,
    pub server: SpectatorServer,
}

impl E2EHarness {
    /// Launch Godot headless, create SpectatorServer, connect, handshake.
    pub async fn start(scene: &str) -> Result<Self> {
        let godot = GodotProcess::start(scene).await?;

        let state = Arc::new(Mutex::new(SessionState::default()));
        let tcp_state = state.clone();
        let port = godot.port();

        // Spawn TCP client loop (will connect + handshake)
        tokio::spawn(async move {
            tcp::tcp_client_loop(tcp_state, port).await;
        });

        // Wait for handshake to complete
        wait_for_connected(&state, Duration::from_secs(5)).await?;

        let server = SpectatorServer::new(state);
        Ok(Self { godot, server })
    }

    /// Call an MCP tool and return the parsed JSON result.
    pub async fn call_tool(&self, name: &str, params: Value) -> Result<Value> {
        // Dispatch via tool router
        ...
    }
}
```

### E2E Test Cases

These test against the real Godot scene, so assertions are on actual game data.

#### Connection
- `test_e2e_connects_to_godot` — handshake completes, session ID assigned,
  project name matches, dimensions = 3D
- `test_e2e_godot_restart_reconnects` — kill and restart Godot, server
  reconnects and re-handshakes

#### spatial_snapshot — real data
- `test_e2e_snapshot_summary` — summary shows correct group counts
  (1 player, 2 enemies, 2 walls, 2 collectibles)
- `test_e2e_snapshot_standard` — per-entity data has correct positions
  (Player at origin, Scout at (5, 0, -3), etc.)
- `test_e2e_snapshot_bearings` — bearing from camera to Player is correct
  (camera at (0,5,10) looking at origin → Player is "ahead")
- `test_e2e_snapshot_group_filter` — `groups: ["enemies"]` returns exactly
  Scout and Tank
- `test_e2e_snapshot_state` — exported vars visible (health, speed)

#### spatial_inspect — real data
- `test_e2e_inspect_player` — inspect Player, verify transform at origin,
  health=100, script path, children (CollisionShape3D, MeshInstance3D)
- `test_e2e_inspect_spatial_context` — nearby entities list, camera visibility

#### scene_tree — real hierarchy
- `test_e2e_scene_tree_roots` — top-level node is TestScene3D
- `test_e2e_scene_tree_subtree` — full tree matches expected structure
- `test_e2e_scene_tree_find_enemies` — find by group "enemies" returns 2 nodes

#### spatial_action — real mutations
- `test_e2e_action_pause_unpause` — pause tree, verify paused state, unpause
- `test_e2e_action_teleport` — teleport Scout to (10, 0, 0), verify with
  subsequent snapshot
- `test_e2e_action_set_property` — set Scout health to 50, verify with inspect
- `test_e2e_action_advance_frames` — pause, advance 5 frames, verify frame
  counter advanced by 5

#### spatial_query — real physics
- `test_e2e_query_raycast` — raycast from Player to Scout, check if clear
- `test_e2e_query_raycast_blocked` — raycast through a wall, verify blocked
- `test_e2e_query_nearest` — nearest 2 nodes to Player

#### spatial_delta — real changes
- `test_e2e_delta_after_teleport` — snapshot, teleport Scout, delta shows
  Scout in `moved`
- `test_e2e_delta_after_property_change` — snapshot, set health, delta shows
  `state_changed`

#### 2D scene
- `test_e2e_2d_snapshot` — 2D scene returns 2D positions, 2D bearings
- `test_e2e_2d_dimensions` — handshake reports dimensions=2

#### recording — real capture
- `test_e2e_recording_start_stop` — start recording, advance frames, stop,
  verify frame count
- `test_e2e_recording_snapshot_at` — start, advance, stop, query snapshot_at
  a specific frame

---

## File Organization

```
crates/spectator-server/
    tests/
        integration/
            mod.rs               → conditional compilation, shared setup
            tcp_mock.rs          → Layer 1 test module
            e2e.rs               → Layer 2 test module (gated behind feature/env)
        support/
            mod.rs
            mock_addon.rs        → MockAddon implementation
            fixtures.rs          → Canned scene data
            harness.rs           → TestHarness (mock layer)
            godot_process.rs     → GodotProcess launcher
            e2e_harness.rs       → E2EHarness (real Godot)
tests/
    godot-project/               → Minimal Godot project for E2E
        project.godot
        test_scene_3d.tscn
        test_scene_3d.gd
        test_scene_2d.tscn
        test_scene_2d.gd
        addons/spectator/        → symlink
```

### Feature Gating

```toml
# crates/spectator-server/Cargo.toml
[features]
integration-tests = []  # enables tcp mock tests (no external deps)
e2e-tests = []          # enables real-Godot tests (needs godot binary)

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"
```

Running tests:

```bash
# Unit tests only (current behavior)
cargo test --workspace

# Unit + TCP mock integration tests
cargo test --workspace --features integration-tests

# Full E2E tests (needs Godot binary)
cargo test --workspace --features e2e-tests

# Just E2E
cargo test -p spectator-server --features e2e-tests -- e2e
```

### Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `GODOT_BIN` | Path to Godot binary | `godot` (from PATH) |
| `SPECTATOR_TEST_PORT` | Base port for test TCP listeners | auto (ephemeral) |
| `SPECTATOR_E2E_TIMEOUT` | Max seconds to wait for Godot startup | `10` |

---

## CI Integration

### GitHub Actions — Layer 1 Only (immediate)

TCP mock tests need no external dependencies. Add to existing CI:

```yaml
# .github/workflows/ci.yml (add to existing 'check' job)
- name: Run integration tests
  run: cargo test --workspace --features integration-tests
```

### GitHub Actions — Layer 2 (future)

E2E tests require Godot. Use the `chickensoft-games/setup-godot` action:

```yaml
e2e-tests:
  name: E2E Integration Tests
  runs-on: ubuntu-latest
  needs: check
  steps:
    - uses: actions/checkout@v4

    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable

    - name: Install Godot 4.6
      uses: chickensoft-games/setup-godot@v2
      with:
        version: 4.6.1
        use-dotnet: false

    - name: Build GDExtension
      run: cargo build -p spectator-godot

    - name: Copy GDExtension to test project
      run: |
        mkdir -p tests/godot-project/addons/spectator/bin/linux/
        cp target/debug/libspectator_godot.so tests/godot-project/addons/spectator/bin/linux/
        cp addons/spectator/*.gd tests/godot-project/addons/spectator/
        cp addons/spectator/plugin.cfg tests/godot-project/addons/spectator/
        cp addons/spectator/spectator.gdextension tests/godot-project/addons/spectator/

    - name: Import Godot project (generates .godot/ cache)
      run: godot --headless --import --path tests/godot-project --quit

    - name: Run E2E tests
      run: cargo test -p spectator-server --features e2e-tests
      env:
        GODOT_BIN: godot
```

---

## Implementation Order

1. **Test support crate scaffolding** — `tests/support/mod.rs`, mock addon,
   fixtures
2. **MockAddon** — TCP listener with handshake + query dispatch
3. **TestHarness** — server creation, tool invocation helper
4. **Layer 1 tests** — start with snapshot (most complex), then simpler tools
5. **Test Godot project** — create `tests/godot-project/` with scenes
6. **GodotProcess** — headless launcher with port management
7. **E2EHarness** — wiring GodotProcess + SpectatorServer
8. **Layer 2 tests** — mirror Layer 1 structure against real Godot
9. **CI** — add feature flags to workflow

Steps 1-4 can be done without Godot. Steps 5-8 require a working GDExtension.

---

## Key Design Decisions

### Why not test through MCP stdio transport?

The stdio transport requires a child process and MCP client SDK. Testing
handlers directly is simpler, faster, and gives the same coverage of the
logic under test. The MCP transport layer is tested by rmcp's own test suite.

### Why an in-repo test project instead of using the cosmic showcase?

The cosmic showcase is designed for visual demonstration — it has animations,
tweens, timers, camera movement, and randomized asteroid spawning. This makes
it non-deterministic and slow to stabilize. A purpose-built test scene with
fixed positions and no animations gives deterministic, fast, reliable tests.

### Why ephemeral ports?

Hardcoded ports cause test failures when running in parallel or when another
process holds the port. Binding to port 0 lets the OS assign a free port.
The mock/Godot process reports its port back to the test harness.

### Why feature-gated instead of always-on?

- `integration-tests`: The mock tests add dev-dependency compile time but are
  safe to run everywhere. Could be always-on once stable.
- `e2e-tests`: Require Godot binary. Must be opt-in to avoid CI failures on
  environments without Godot. Local developers run them manually.

### Handling Godot's addon port

The test project must read `SPECTATOR_PORT` env var to override the default
9077. The addon already reads `spectator/connection/port` from Project
Settings — we can either:

1. **Override in `project.godot`** — set port to a known value per test
2. **Use env var in runtime.gd** — add a `SPECTATOR_PORT` env var check
   before reading Project Settings

Option 2 is preferred: add a 3-line env var check to `runtime.gd` that
takes precedence over the Project Settings port. This is useful for testing
*and* for advanced users who want to run multiple Godot instances.

```gdscript
# In runtime.gd _ready():
var port: int = OS.get_environment("SPECTATOR_PORT").to_int()
if port == 0:
    port = ProjectSettings.get_setting("spectator/connection/port", 9077)
```

### Determinism and timing

- `--fixed-fps 60` makes Godot process exactly 60 physics frames per second,
  regardless of system speed. Combined with `--headless` (no rendering), this
  gives deterministic physics behavior.
- Tests that check frame-dependent behavior (delta, recording) use
  `advance_frames` to control time precisely.
- TCP communication has natural timing uncertainties. Tests use `wait_for`
  helpers with reasonable timeouts (5s default) rather than hard sleeps.
- All position assertions use approximate equality (`(a - b).abs() < 0.01`)
  to account for floating-point differences across platforms.

### Test isolation

Each E2E test gets its own Godot process on its own port. This prevents state
leakage between tests but is slower. For Layer 1 (mock) tests, each test gets
its own MockAddon + SpectatorServer, which is fast enough to not matter.

If E2E tests become too slow, we can share a single Godot process across a
test module and use `spatial_action` to reset state between tests (teleport
nodes back to starting positions, reset properties).
