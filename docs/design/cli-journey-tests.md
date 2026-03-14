# Design: Comprehensive CLI Journey Tests

## Overview

Add multi-step journey tests that exercise the `stage` and `director` CLI
binaries against real Godot scenes. These tests validate the full binary path:
argument parsing → connection/process management → tool dispatch → JSON output
→ exit codes. They complement the existing E2E harness journeys (which call
Rust handler functions directly) by proving the CLI subprocess interface works
end-to-end.

**Current state:**
- Stage CLI: 5 basic arg-validation tests in `crates/stage-server/tests/cli_binary.rs` (version, no-args, unknown tool, invalid JSON, no Godot). Zero journey tests.
- Director CLI: 4 tests in `tests/director-tests/src/test_cli.rs` (create+read, node add, missing project, invalid JSON). Zero multi-step CLI journeys.

**Goal:** Mirror the coverage of `e2e_journeys.rs` (stage) and `test_journey*.rs` (director) but through the CLI binary subprocess interface.

## Implementation Units

### Unit 1: StageCliFixture

**File**: `crates/stage-server/tests/support/cli_fixture.rs`

```rust
use super::godot_process::GodotProcess;
use serde_json::Value;
use std::process::Command;

/// Result from a CLI invocation — either parsed JSON or structured error.
pub enum CliResult {
    /// Exit 0 — stdout parsed as JSON.
    Ok(Value),
    /// Exit non-zero — stdout parsed as JSON error envelope.
    Err {
        exit_code: i32,
        error: Value,
    },
}

impl CliResult {
    pub fn unwrap_data(self) -> Value { /* panic on Err */ }
    pub fn unwrap_err(self) -> (i32, Value) { /* panic on Ok */ }
    pub fn is_ok(&self) -> bool { /* ... */ }
}

/// CLI fixture that manages a Godot process and shells out to the `stage` binary.
pub struct StageCliFixture {
    godot: GodotProcess,
    port: u16,
}

impl StageCliFixture {
    /// Start Godot headless with a test scene. Reuses GodotProcess.
    pub async fn start_3d() -> anyhow::Result<Self> {
        let godot = GodotProcess::start_3d().await?;
        let port = godot.port();
        Ok(Self { godot, port })
    }

    pub async fn start_2d() -> anyhow::Result<Self> {
        let godot = GodotProcess::start_2d().await?;
        let port = godot.port();
        Ok(Self { godot, port })
    }

    /// Invoke `stage <tool> '<json>'` as a subprocess.
    /// Sets THEATRE_PORT to the fixture's port.
    /// Returns CliResult based on exit code and stdout.
    pub fn run(&self, tool: &str, params: Value) -> anyhow::Result<CliResult> {
        let bin = env!("CARGO_BIN_EXE_stage");
        let output = Command::new(bin)
            .args([tool, &params.to_string()])
            .env("THEATRE_PORT", self.port.to_string())
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let code = output.status.code().unwrap_or(-1);
        let parsed: Value = serde_json::from_str(stdout.trim())?;
        if code == 0 {
            Ok(CliResult::Ok(parsed))
        } else {
            Ok(CliResult::Err { exit_code: code, error: parsed })
        }
    }

    /// Wait for N physics frames (blocking sleep — test runs on tokio runtime).
    pub async fn wait_frames(&self, n: u32) {
        let ms = (n as u64 * 1000) / 60 + 50;
        tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
    }

    pub fn port(&self) -> u16 { self.port }

    /// Access Godot stderr for debugging failures.
    pub fn godot_stderr(&self) -> String { self.godot.stderr_output() }
}
// GodotProcess Drop kills Godot automatically.
```

**Implementation Notes**:
- Uses `env!("CARGO_BIN_EXE_stage")` — cargo builds the binary automatically when running tests in `stage-server`.
- `THEATRE_PORT` env var tells the stage CLI which port to connect to.
- `run()` is synchronous (subprocess blocks) — no need for async. But tests are `#[tokio::test]` because `GodotProcess::start` is async and `wait_frames` uses `tokio::time::sleep`.
- All stage CLI output (success and error) goes to stdout as JSON, so we always parse stdout.

**Acceptance Criteria**:
- [ ] `StageCliFixture::start_3d()` launches Godot and makes port available
- [ ] `run("spatial_snapshot", json!({}))` returns `CliResult::Ok(...)` with entities
- [ ] `run("unknown_tool", json!({}))` returns `CliResult::Err { exit_code: 2, .. }`
- [ ] Godot process is killed on fixture drop

---

### Unit 2: Director CliFixture Journey Support

**File**: `tests/director-tests/src/harness.rs` (extend existing `CliFixture`)

The existing `CliFixture` is sufficient for director CLI journeys — no structural
changes needed. The new journey test files simply use `CliFixture` as-is.

**Acceptance Criteria**:
- [ ] Existing `CliFixture::run()` works for multi-step scenarios (it already does — stateless subprocess per call)

---

### Unit 3: Stage CLI Journey Tests

**File**: `crates/stage-server/tests/cli_journeys.rs`

```rust
mod support;

use serde_json::json;
use support::cli_fixture::{StageCliFixture, CliResult};
```

**Journey 1: `cli_journey_explore_scene`** — mirrors `journey_explore_scene` from `e2e_journeys.rs`

Steps:
1. `stage scene_tree '{"action":"roots"}'` → roots array non-empty
2. `stage spatial_snapshot '{"detail":"summary"}'` → non-null
3. `stage spatial_snapshot '{"detail":"standard"}'` → Player at ~(0,0,0), Scout at ~(5,0,-3)
4. `stage spatial_inspect '{"node":"Enemies/Scout"}'` → class, properties.health=80
5. `stage spatial_query '{"query_type":"nearest","from":[0,0,0],"k":2}'` → results with non-negative distances

**Journey 2: `cli_journey_mutate_and_observe`** — mirrors `journey_debug_spatial_bug`

Steps:
1. `spatial_snapshot {"detail":"standard"}` → baseline, note Scout position
2. `spatial_action {"action":"teleport","node":"Enemies/Scout","position":[0,0,0]}` → ack
3. `wait_frames(5)`
4. `spatial_delta {}` → Scout in "moved" array
5. `spatial_snapshot {"detail":"standard"}` → Scout now at ~(0,0,0)
6. `spatial_action {"action":"set_property","node":"Enemies/Scout","property":"health","value":25}` → ack
7. `wait_frames(2)`
8. `spatial_inspect {"node":"Enemies/Scout"}` → health=25

**Journey 3: `cli_journey_2d_scene`** — mirrors `journey_2d_scene`

Steps:
1. `spatial_snapshot {"detail":"standard","radius":500.0}` → entities with 2-element positions
2. `spatial_query {"query_type":"nearest","from":[0,0],"k":2}` → results
3. `spatial_inspect {"node":"Player"}` → no elevation field
4. `spatial_action {"action":"teleport","node":"Player","position":[100,50]}` → ack
5. `wait_frames(3)`
6. `spatial_snapshot {"detail":"standard","radius":500.0,"include_offscreen":true}` → Player at ~(100,50)

**Journey 4: `cli_journey_config_and_watch`** — exercises config + watch tools

Steps:
1. `spatial_config {"action":"get"}` → returns current config
2. `spatial_config {"action":"set","tracking_radius":100.0}` → ack
3. `spatial_watch {"action":"subscribe","node":"Enemies/Scout","track":["position"]}` → watch_id
4. `wait_frames(10)`
5. `spatial_watch {"action":"poll","watch_id":"<id>"}` → events array (may be empty if no change)
6. `spatial_watch {"action":"unsubscribe","watch_id":"<id>"}` → ack

**Journey 5: `cli_journey_error_handling`** — validates error paths through CLI

Steps:
1. `spatial_inspect {"node":"NonExistent/Node"}` → exit 1, error JSON with "tool_error"
2. `spatial_query {"query_type":"nearest"}` → exit 1 (missing `from`)
3. `spatial_action {"action":"teleport"}` → exit 1 (missing `node`)

```rust
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn cli_journey_explore_scene() {
    let f = StageCliFixture::start_3d().await
        .expect("Failed to start Godot 3D scene");

    // Step 1: scene_tree roots
    let roots = f.run("scene_tree", json!({"action": "roots"}))
        .unwrap().unwrap_data();
    assert!(roots["roots"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "roots should be non-empty");

    // Step 2: summary snapshot
    let summary = f.run("spatial_snapshot", json!({"detail": "summary"}))
        .unwrap().unwrap_data();
    assert!(!summary.is_null());

    // Step 3: standard snapshot — real positions
    let snapshot = f.run("spatial_snapshot", json!({"detail": "standard"}))
        .unwrap().unwrap_data();
    let entities = snapshot["entities"].as_array()
        .expect("entities array");
    let player = entities.iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Player")).unwrap_or(false))
        .expect("Player in snapshot");
    let pos = player["global_position"].as_array().expect("position");
    assert!(pos[0].as_f64().unwrap_or(999.0).abs() < 1.0, "Player X ~0");

    let scout = entities.iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false))
        .expect("Scout in snapshot");
    let scout_pos = scout["global_position"].as_array().expect("position");
    assert!((scout_pos[0].as_f64().unwrap_or(0.0) - 5.0).abs() < 1.0, "Scout X ~5");

    // Step 4: inspect Scout
    let inspect = f.run("spatial_inspect", json!({"node": "Enemies/Scout"}))
        .unwrap().unwrap_data();
    assert!(inspect["class"].is_string() || inspect["node_path"].is_string());
    if let Some(health) = inspect["properties"].get("health") {
        assert!((health.as_f64().unwrap_or(0.0) - 80.0).abs() < 0.1);
    }

    // Step 5: nearest query
    let query = f.run("spatial_query", json!({
        "query_type": "nearest", "from": [0.0, 0.0, 0.0], "k": 2
    })).unwrap().unwrap_data();
    let results = query["results"].as_array().expect("results");
    assert!(!results.is_empty());
    for r in results {
        assert!(r["distance"].as_f64().unwrap_or(-1.0) >= 0.0);
    }
}
```

**Implementation Notes**:
- Each journey is a `#[tokio::test]` with `#[ignore = "requires Godot binary"]`.
- The fixture starts one Godot instance per test. CLI invocations connect and disconnect within each `run()` call — the Godot addon accepts multiple connections sequentially.
- `wait_frames` is needed between mutation and observation because the CLI connects fresh each time (no persistent session state).
- Delta/watch tests will have a quirk: each CLI invocation is a separate session, so `spatial_delta` compares against an empty baseline (first call). The test should call snapshot once (to establish baseline state in the server), then do the mutation, then call snapshot+delta in the same CLI session... **Wait** — each CLI call is an independent session. Delta compares against the *session's* previous snapshot. Since each CLI call is a new session, delta will always show everything as "new". This is an important behavioral difference from MCP mode.

**Key insight: Per-call sessions in CLI mode mean stateful tools (delta, watch, clips) behave differently.** Each CLI invocation creates a fresh session. This means:
- `spatial_delta` always returns a full diff (no prior baseline) — effectively equivalent to a snapshot.
- `spatial_watch` subscribe + poll can't work across CLI calls (watch_id is session-scoped).
- `clips` status/list/save work fine (clip DB is on-disk, not session-scoped).

This simplifies Journey 4 (config+watch) — we should **skip watch subscribe/poll** in CLI journey tests (it's inherently an MCP-mode feature). Config `get`/`set` may also be session-scoped. We should test what *does* work: clips lifecycle via CLI.

**Revised Journey 4: `cli_journey_clips_lifecycle`** — exercises clips through CLI

Steps:
1. `wait_frames(60)` → let buffer accumulate
2. `clips {"action":"status"}` → dashcam_enabled, state="buffering"
3. `clips {"action":"save","marker_label":"cli_test"}` → clip_id
4. `clips {"action":"list"}` → clip present in list
5. `clips {"action":"delete","clip_id":"<id>"}` → result="ok"
6. `clips {"action":"list"}` → clip gone

**Acceptance Criteria**:
- [ ] `cli_journey_explore_scene` — scene_tree, snapshot, inspect, query all return valid JSON via CLI
- [ ] `cli_journey_mutate_and_observe` — teleport via CLI, verify position change in subsequent snapshot
- [ ] `cli_journey_2d_scene` — 2D positions (2-element arrays), teleport, verify
- [ ] `cli_journey_clips_lifecycle` — save/list/delete clips via CLI
- [ ] `cli_journey_error_handling` — invalid nodes, missing params produce exit 1 with structured error JSON
- [ ] All tests marked `#[ignore = "requires Godot binary"]`

---

### Unit 4: Director CLI Journey Tests

**File**: `tests/director-tests/src/test_cli_journey.rs`

```rust
use crate::harness::{CliFixture, DirectorFixture, OperationResultExt, assert_approx};
use serde_json::json;
```

**Journey 1: `cli_journey_build_scene`** — create + populate + verify entirely via CLI

Steps:
1. `scene_create` → CharacterBody2D root
2. `node_add` → Sprite2D "Sprite" child
3. `node_add` → CollisionShape2D "Collision" child
4. `shape_create` → CapsuleShape2D on Collision
5. `node_set_properties` → position (200, 300) on root
6. `scene_read` → verify full tree: root type, children, position
7. `node_remove` → remove Sprite
8. `scene_read` → verify only Collision remains

**Journey 2: `cli_journey_multi_scene_composition`** — scene instancing + reparenting via CLI

Steps:
1. `scene_create` enemy scene (CharacterBody2D)
2. `node_add` Sprite2D child to enemy
3. `scene_create` level scene (Node2D)
4. `node_add` "Enemies" group node
5. `node_add` "Staging" group node
6. `scene_add_instance` enemy into Staging
7. `node_reparent` enemy from Staging to Enemies
8. `scene_read` → verify Staging empty, Enemies has the enemy instance
9. `scene_list {"directory":"tmp"}` → both scenes present

**Journey 3: `cli_journey_animation_workflow`** — create + build + read animation via CLI

Steps:
1. `scene_create` → Node2D scene
2. `node_add` → AnimationPlayer
3. `animation_create` → "walk" animation (1.0s, loop)
4. `animation_add_track` → property track for position
5. `animation_read` → verify track count, duration, loop

**Journey 4: `cli_journey_physics_and_signals`** — set layers + connect signals via CLI

Steps:
1. `scene_create` → Node2D scene
2. `node_add` → Area2D
3. `node_add` → CollisionShape2D under Area2D
4. `physics_set_layers` → set collision_layer and collision_mask
5. `scene_read` → verify layers set
6. `signal_connect` → connect "body_entered" to a method
7. `signal_list` → verify connection exists

**Journey 5: `cli_journey_batch_operations`** — batch multiple ops in one call via CLI

Steps:
1. `scene_create` → base scene
2. `batch` → add 3 nodes + set properties in single batch call
3. `scene_read` → verify all 3 nodes with properties

**Journey 6: `cli_journey_error_cases`** — validate error handling via CLI

Steps:
1. `scene_read` with non-existent scene → error
2. `node_add` with non-existent parent → error
3. `node_remove` with non-existent node → error

```rust
#[test]
#[ignore = "requires Godot binary"]
fn cli_journey_build_scene() {
    let cli = CliFixture::new();
    let scene = DirectorFixture::temp_scene_path("cli_j_build");

    // 1. Create scene
    let data = cli.run("scene_create", json!({
        "scene_path": scene, "root_type": "CharacterBody2D"
    })).unwrap().unwrap_data();
    assert_eq!(data["root_type"], "CharacterBody2D");

    // 2. Add Sprite2D
    cli.run("node_add", json!({
        "scene_path": scene, "node_type": "Sprite2D", "node_name": "Sprite"
    })).unwrap().unwrap_data();

    // 3. Add CollisionShape2D
    cli.run("node_add", json!({
        "scene_path": scene, "node_type": "CollisionShape2D", "node_name": "Collision"
    })).unwrap().unwrap_data();

    // 4. Create shape
    cli.run("shape_create", json!({
        "shape_type": "CapsuleShape2D",
        "shape_params": {"radius": 16.0, "height": 48.0},
        "scene_path": scene,
        "node_path": "Collision"
    })).unwrap().unwrap_data();

    // 5. Set position
    cli.run("node_set_properties", json!({
        "scene_path": scene, "node_path": ".",
        "properties": {"position": {"x": 200, "y": 300}}
    })).unwrap().unwrap_data();

    // 6. Read back
    let tree = cli.run("scene_read", json!({"scene_path": scene}))
        .unwrap().unwrap_data();
    let root = &tree["root"];
    assert_eq!(root["type"], "CharacterBody2D");
    assert_approx(root["properties"]["position"]["x"].as_f64().unwrap(), 200.0);
    let children = root["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);

    // 7. Remove Sprite
    cli.run("node_remove", json!({
        "scene_path": scene, "node_path": "Sprite"
    })).unwrap().unwrap_data();

    // 8. Verify removal
    let tree = cli.run("scene_read", json!({"scene_path": scene}))
        .unwrap().unwrap_data();
    let children = tree["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "Collision");
}
```

**Implementation Notes**:
- Director `CliFixture` already injects `project_path` automatically.
- Each `CliFixture::run()` spawns a fresh Godot process, so tests are isolated.
- Temp scene paths use `DirectorFixture::temp_scene_path()` for cleanup.
- All tests `#[ignore = "requires Godot binary"]`.

**Acceptance Criteria**:
- [ ] `cli_journey_build_scene` — full node lifecycle via CLI
- [ ] `cli_journey_multi_scene_composition` — instancing + reparenting via CLI
- [ ] `cli_journey_animation_workflow` — animation CRUD via CLI
- [ ] `cli_journey_physics_and_signals` — physics layers + signal wiring via CLI
- [ ] `cli_journey_batch_operations` — batch operations via CLI
- [ ] `cli_journey_error_cases` — structured errors via CLI

---

### Unit 5: Register New Test Modules

**File**: `crates/stage-server/tests/cli_journeys.rs` — new integration test file (cargo discovers it automatically)

**File**: `tests/director-tests/src/lib.rs` — add module registration

```rust
// Add to lib.rs:
#[cfg(test)]
mod test_cli_journey;
```

**File**: `crates/stage-server/tests/support/mod.rs` — add cli_fixture module

```rust
// Add to mod.rs:
pub mod cli_fixture;
```

**Acceptance Criteria**:
- [ ] `cargo test -p stage-server --test cli_journeys -- --list` lists all stage CLI journeys
- [ ] `cargo test -p director-tests -- --list` includes `test_cli_journey::*`

---

## Implementation Order

1. **Unit 1**: `StageCliFixture` — the foundational harness
2. **Unit 5**: Module registration (support/mod.rs, cli_journeys.rs stub, lib.rs)
3. **Unit 3**: Stage CLI journey tests (using the fixture)
4. **Unit 4**: Director CLI journey tests (using existing `CliFixture`)
5. **Unit 2**: Verify existing `CliFixture` works (no changes expected)

## Testing

### Running the Tests

```bash
# Build binaries first (required for CLI subprocess tests)
cargo build --workspace

# Run stage CLI journeys
cargo test -p stage-server --test cli_journeys -- --ignored --nocapture

# Run director CLI journeys
cargo test -p director-tests test_cli_journey -- --ignored --nocapture

# Run ALL tests including new journeys
cargo test --workspace -- --include-ignored
```

### What Each Journey Validates

| Journey | Binary | Tools Exercised | Key Validation |
|---------|--------|-----------------|----------------|
| `cli_journey_explore_scene` | stage | scene_tree, snapshot, inspect, query | Read-only observation via CLI |
| `cli_journey_mutate_and_observe` | stage | snapshot, action, delta, inspect | Mutation + observation round-trip |
| `cli_journey_2d_scene` | stage | snapshot, query, inspect, action | 2D coordinate handling via CLI |
| `cli_journey_clips_lifecycle` | stage | clips (status, save, list, delete) | Clip DB persistence across CLI sessions |
| `cli_journey_error_handling` | stage | inspect, query, action (invalid params) | Structured error JSON, exit codes |
| `cli_journey_build_scene` | director | scene_create, node_add, shape_create, node_set_properties, scene_read, node_remove | Full node lifecycle via CLI |
| `cli_journey_multi_scene_composition` | director | scene_create, node_add, scene_add_instance, node_reparent, scene_read, scene_list | Scene composition via CLI |
| `cli_journey_animation_workflow` | director | scene_create, node_add, animation_create, animation_add_track, animation_read | Animation CRUD via CLI |
| `cli_journey_physics_and_signals` | director | scene_create, node_add, physics_set_layers, scene_read, signal_connect, signal_list | Physics + signals via CLI |
| `cli_journey_batch_operations` | director | scene_create, batch, scene_read | Batch operations via CLI |
| `cli_journey_error_cases` | director | scene_read, node_add, node_remove (invalid) | Error handling via CLI |

## Verification Checklist

```bash
# 1. All tests compile
cargo test --workspace --no-run

# 2. Non-E2E tests still pass
cargo test --workspace

# 3. CLI binary tests pass (no Godot needed)
cargo test -p stage-server --test cli_binary

# 4. Full E2E + CLI journeys pass (requires Godot + deployed GDExtension)
theatre deploy tests/godot-project
cargo test --workspace -- --include-ignored --nocapture

# 5. Clippy clean
cargo clippy --workspace
```
