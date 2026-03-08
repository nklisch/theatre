# Design: E2E Journey Tests

## Overview

Complex, multi-step integration tests that simulate real developer/agent
debugging journeys. Each test exercises 5-10 tool calls in sequence, crossing
boundaries between spatial indexing, delta tracking, watches, recording, config,
and actions. The goal is to catch bugs that only appear when tools interact
across shared state — the kind of bugs unit tests and single-tool integration
tests miss.

These tests are **few but deep**: 5 journey tests, each telling a story.

## Key Design Decision: StatefulMockWorld

The existing `QueryHandler` closure is stateless per-test or uses ad-hoc
`Arc<Mutex<T>>` state. Journey tests need a richer simulation: entities that
move, properties that change, signals that fire — all in response to actions
the test takes. Rather than hand-rolling `Arc<Mutex<...>>` per test, we
introduce a `StatefulMockWorld` that simulates evolving game state and produces
a `QueryHandler` from it.

This is the single biggest infrastructure addition. Everything else is test
code.

---

## Implementation Units

### Unit 1: StatefulMockWorld

**File**: `crates/spectator-server/tests/support/world.rs`

```rust
use super::fixtures::EntityData;
use super::mock_addon::QueryHandler;
use serde_json::{json, Value};
use spectator_protocol::query::{PerspectiveData, SnapshotResponse};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mutable game world for journey tests.
/// Entities can be moved, properties changed, and signals queued.
/// Produces a QueryHandler that serves live state for snapshot/inspect/action queries.
pub struct StatefulMockWorld {
    inner: Arc<Mutex<WorldState>>,
}

struct WorldState {
    frame: u64,
    timestamp_ms: u64,
    perspective: PerspectiveData,
    entities: Vec<EntityData>,
    /// Signals queued by set_signal(); drained on get_snapshot_data.
    pending_signals: Vec<Value>,
    /// Active recording state.
    recording: Option<RecordingState>,
    /// History of method calls received (for assertion).
    call_log: Vec<(String, Value)>,
}

struct RecordingState {
    id: String,
    name: String,
    started_at_frame: u64,
    frames_captured: u32,
}

impl StatefulMockWorld {
    /// Create a world from a fixture scene (e.g., mock_scene_3d()).
    pub fn from_scene(scene: SnapshotResponse) -> Self { ... }

    /// Move an entity to a new position. Panics if entity not found.
    pub fn move_entity(&self, path: &str, position: Vec<f64>) { ... }

    /// Set a state property on an entity.
    pub fn set_state(&self, path: &str, key: &str, value: Value) { ... }

    /// Queue a signal emission (will appear in next snapshot's signals_recent).
    pub fn queue_signal(&self, node: &str, signal: &str, args: Vec<Value>) { ... }

    /// Add a new entity to the world.
    pub fn add_entity(&self, entity: EntityData) { ... }

    /// Remove an entity by path.
    pub fn remove_entity(&self, path: &str) { ... }

    /// Advance the frame counter by N frames.
    pub fn advance_frames(&self, n: u64) { ... }

    /// Get the current frame number.
    pub fn frame(&self) -> u64 { ... }

    /// Get the call log for assertions ("what methods did the server ask for?").
    pub fn call_log(&self) -> Vec<(String, Value)> { ... }

    /// Clear the call log.
    pub fn clear_call_log(&self) { ... }

    /// Build a QueryHandler that serves live state from this world.
    ///
    /// Handles: get_snapshot_data, get_node_inspect, execute_action,
    /// recording_start, recording_stop, recording_status, get_scene_tree.
    pub fn handler(&self) -> QueryHandler { ... }
}
```

**Implementation Notes**:

The handler closure captures `Arc<Mutex<WorldState>>` and dispatches on method:

- `get_snapshot_data` → builds `SnapshotResponse` from current entities, drains
  pending signals into `signals_recent`, advances frame by 1.
- `get_node_inspect` → finds entity by path, returns `NodeInspectResponse` with
  current state.
- `execute_action` → dispatches on `params["action"]`:
  - `"teleport"` → updates entity position, returns previous position
  - `"set_property"` → updates entity state, returns previous value
  - `"pause"` / `"unpause"` → toggles a paused flag, returns ack
  - `"call_method"` → returns `json!({"result": "ok"})`
- `get_scene_tree` → returns flat tree of entity paths and classes.
- `recording_start` → creates RecordingState, returns id.
- `recording_stop` → clears RecordingState, returns frame count.
- `recording_status` → returns active state.
- Unknown methods → `Err(("unknown_method", ...))`

All method calls are logged to `call_log` for assertion.

**Acceptance Criteria**:
- [ ] `move_entity` changes position visible in next `get_snapshot_data`
- [ ] `set_state` changes property visible in next `get_node_inspect`
- [ ] `execute_action(teleport)` mutates world state
- [ ] `call_log` records all method+params received
- [ ] Multiple tests can create independent `StatefulMockWorld` instances

---

### Unit 2: Journey Test — Bug Investigation

**File**: `crates/spectator-server/tests/journeys.rs`

**Story**: An agent is told "the enemy clips through the east wall." It takes a
snapshot, inspects the scene tree, queries the spatial relationship between the
enemy and wall, teleports the enemy to reproduce, takes another snapshot to
confirm, then checks delta to see the movement.

```rust
/// Journey: Investigating an enemy clipping through a wall.
///
/// Tool sequence:
///   1. spatial_snapshot(standard) → get entity positions
///   2. scene_tree(root: "walls") → find wall nodes
///   3. spatial_query(relationship, enemy ↔ wall) → check distance
///   4. spatial_action(teleport enemy to wall position) → reproduce clip
///   5. spatial_snapshot(standard) → confirm new position
///   6. spatial_delta() → verify enemy shows as "moved"
///   7. spatial_inspect(enemy) → check physics state at wall
#[tokio::test]
async fn journey_investigate_wall_clip() { ... }
```

**Implementation Notes**:

Setup: `StatefulMockWorld::from_scene(mock_scene_3d())`. Scout starts at
`[0, 0, -5]`, EastWall at `[3, 0, 0]`.

Step-by-step with assertions:

1. **Snapshot**: Assert Scout and EastWall both present. Assert Scout's
   distance from origin is ~5.0.

2. **Scene tree** with root filter: Assert wall paths returned.

3. **Spatial query** (radius from wall position): Assert Scout is NOT within
   radius 1.0 of the wall (they're 5.8m apart).

4. **Teleport** Scout to `[3.0, 0.0, 0.0]` (same as wall): Assert success,
   assert previous_position returned. World state updates.

5. **Snapshot**: Assert Scout now at `[3, 0, 0]`. Assert Scout is within 0.5m
   of EastWall.

6. **Delta**: Assert Scout appears in `moved` array. Assert position change
   from `[0, 0, -5]` to `[3, 0, 0]`.

7. **Inspect** Scout: Assert path, class, position match. Assert state dict
   contains `health: 80`.

Key boundary tested: **action → snapshot → delta coherence**. The spatial index
must update after teleport; delta must compare against the pre-teleport
baseline.

**Acceptance Criteria**:
- [ ] All 7 tool calls succeed in sequence
- [ ] Teleport mutates world state visible in subsequent snapshot
- [ ] Delta correctly reflects the teleport as movement
- [ ] Spatial query uses the updated index after snapshot refresh

---

### Unit 3: Journey Test — Recording and Playback Analysis

**Story**: The developer says "the enemy's health drops to zero but it doesn't
die." The agent starts recording, the world evolves (health decreases over
frames), the agent stops recording, then queries the recording to find the
frame where health hit zero.

```rust
/// Journey: Recording a health drain and analyzing the timeline.
///
/// Tool sequence:
///   1. recording(start) → begin capture
///   2. spatial_snapshot(standard) → baseline (health=80)
///   3. [world evolves: health drops 80→60→40→20→0 across frames]
///   4. spatial_snapshot(standard) → mid-session check (health=20)
///   5. spatial_delta() → see health in state_changed
///   6. recording(stop) → end capture
///   7. recording(status) → confirm not active
///   8. spatial_config(read) → verify session state not corrupted
#[tokio::test]
async fn journey_recording_health_drain() { ... }
```

**Implementation Notes**:

Between tool calls, the test mutates world state via
`world.set_state("enemies/Scout", "health", ...)` and
`world.advance_frames(10)` to simulate time passing.

The key boundary: **recording lifecycle ↔ snapshot ↔ delta ↔ config** all
sharing the same session state. A bug in recording start/stop could corrupt
the SessionState mutex, breaking subsequent snapshot or config calls.

After recording stop, the test calls `spatial_config` (read mode) to verify
the session state is still intact — not corrupted by the recording lifecycle.

**Acceptance Criteria**:
- [ ] Recording start returns an id; status shows active=true
- [ ] Snapshots during recording reflect evolving state
- [ ] Delta between snapshots detects health change in state_changed
- [ ] Recording stop returns frame count > 0
- [ ] Post-recording status shows active=false
- [ ] Post-recording spatial_config returns valid config (state not corrupted)

---

### Unit 4: Journey Test — Watch-Driven Debugging Loop

**Story**: The agent sets up watches on multiple entities, then polls delta
repeatedly as the world evolves. It tests that watches accumulate correctly,
that clearing one watch doesn't affect others, and that the delta event stream
stays coherent across watch modifications.

```rust
/// Journey: Setting up watches, evolving world, checking deltas.
///
/// Tool sequence:
///   1. spatial_snapshot(standard) → establish baseline
///   2. spatial_watch(add, Player state) → watch_id_1
///   3. spatial_watch(add, Scout state) → watch_id_2
///   4. spatial_watch(list) → verify 2 watches
///   5. [world: Scout health 80→50, Player moves]
///   6. spatial_delta() → Scout state_changed + Player moved
///   7. spatial_watch(remove, watch_id_1) → remove Player watch
///   8. spatial_watch(list) → verify 1 watch remaining
///   9. [world: Scout health 50→10]
///  10. spatial_delta() → Scout state_changed, Player NOT in moved (no change)
///  11. spatial_watch(clear) → remove all
///  12. spatial_watch(list) → verify 0 watches
///  13. spatial_delta() → still works, no crash
#[tokio::test]
async fn journey_watch_driven_debug_loop() { ... }
```

**Implementation Notes**:

The key boundary: **watch lifecycle ↔ delta accumulation ↔ snapshot baseline**.
Watch add/remove/clear must not corrupt the delta engine's baseline state. A
common bug: removing a watch while delta is tracking changes for that entity
causes a panic or stale data.

Between steps 5-6 and 9-10, the test uses `world.move_entity()` and
`world.set_state()` to evolve state, then asserts delta correctly reports
the changes.

Step 13 (delta after clearing all watches) tests that delta still functions
even with zero watches — watches are optional overlays, not required for delta.

**Acceptance Criteria**:
- [ ] Watch add returns unique IDs
- [ ] Watch list reflects current watch set after add/remove/clear
- [ ] Delta reports state changes for watched entities
- [ ] Removing a watch does not corrupt delta baseline
- [ ] Delta works correctly with zero watches
- [ ] 13 sequential tool calls complete without error

---

### Unit 5: Journey Test — Error Recovery Under Load

**Story**: The agent is mid-workflow when things go wrong. It gets an error
from inspect (node not found), retries with a different node, gets another
error from a bad action, then recovers and continues with snapshots and
deltas. The session state must remain consistent throughout.

```rust
/// Journey: Errors mid-workflow don't corrupt session state.
///
/// Tool sequence:
///   1. spatial_snapshot(standard) → establish baseline
///   2. spatial_inspect(node: "/Ghost") → ERROR: node_not_found
///   3. spatial_inspect(node: "enemies/Scout") → SUCCESS
///   4. spatial_action(teleport, "/Ghost", [0,0,0]) → ERROR: node_not_found
///   5. spatial_snapshot(standard) → must still work (state not corrupted)
///   6. spatial_watch(add, Player) → must still work
///   7. spatial_action(teleport, "Player", [10,0,0]) → SUCCESS
///   8. spatial_snapshot(standard) → Player at [10,0,0]
///   9. spatial_delta() → Player moved
///  10. spatial_config(read) → session config intact
#[tokio::test]
async fn journey_error_recovery_under_load() { ... }
```

**Implementation Notes**:

The handler in `StatefulMockWorld` returns errors for unknown paths (like
"/Ghost"). Steps 2 and 4 produce McpError results. The test asserts
`is_err()` for those, then asserts that subsequent calls still succeed.

The key boundary: **error propagation isolation**. An error in the TCP
query/response cycle must not leave the `SessionState` mutex poisoned, the
`oneshot` channels dangling, or the spatial index in a half-updated state.

Steps 5-10 after the errors verify every major subsystem still works:
snapshot (spatial index), watch (watch engine), action (TCP query), delta
(delta engine), config (session state).

**Acceptance Criteria**:
- [ ] Inspect of missing node returns error, does not panic
- [ ] Action on missing node returns error, does not panic
- [ ] Snapshot after errors returns valid entities
- [ ] Watch after errors creates watch successfully
- [ ] Action after errors mutates world state correctly
- [ ] Delta after errors detects movement correctly
- [ ] Config after errors returns valid configuration

---

### Unit 6: Journey Test — 2D/3D Session Isolation

**Story**: Two separate sessions — one 2D, one 3D — run independently. This
tests that the server correctly adapts to scene dimensions from the handshake
and doesn't mix up 2D and 3D logic.

```rust
/// Journey: 2D and 3D sessions behave correctly in isolation.
///
/// This test runs two parallel harnesses to verify that scene dimension
/// detection correctly configures bearings, spatial index, and position
/// format independently.
///
/// Tool sequences (both run sequentially within one test):
///   3D session:
///     1. spatial_snapshot(standard) → entities have [x,y,z] positions
///     2. spatial_query(radius from origin, r=5) → uses R-tree index
///     3. spatial_inspect(Player) → 3D transform
///   2D session:
///     4. spatial_snapshot(standard) → entities have [x,y] positions
///     5. spatial_query(radius from [100,300], r=50) → uses grid index
///     6. spatial_inspect(Player) → 2D transform
#[tokio::test]
async fn journey_2d_3d_session_isolation() { ... }
```

**Implementation Notes**:

Uses `TestHarness::new()` for 3D and `TestHarness::new_2d()` for 2D, each
with their own `StatefulMockWorld`. This tests that the server reads
`scene_dimensions` from the handshake and configures the correct spatial
index type.

The 2D assertions check that positions are 2-element arrays and that no
elevation field appears in bearing data. The 3D assertions check for
3-element positions.

**Acceptance Criteria**:
- [ ] 3D snapshot returns [x,y,z] positions
- [ ] 2D snapshot returns [x,y] positions
- [ ] 3D query uses R-tree spatial index
- [ ] 2D query returns correct entities within radius
- [ ] No cross-contamination between sessions

---

### Unit 7: DebugContext Helper

**File**: `crates/spectator-server/tests/support/debug.rs`

```rust
/// Wrapper around TestHarness that logs every tool call and result for
/// easy debugging when journey tests fail.
///
/// On test failure, the full call trace is included in the panic message.
pub struct DebugContext {
    harness: TestHarness,
    trace: Vec<TraceEntry>,
}

struct TraceEntry {
    step: usize,
    tool: String,
    params: Value,
    result: Result<Value, String>,
    elapsed_ms: u64,
}

impl DebugContext {
    pub fn new(harness: TestHarness) -> Self { ... }

    /// Call a tool and log the call + result. On error, includes full trace
    /// in the error message for debugging.
    pub async fn call(
        &mut self,
        step: usize,
        tool: &str,
        params: Value,
    ) -> Result<Value, McpError> { ... }

    /// Call a tool expecting success. Panics with full trace on failure.
    pub async fn expect(
        &mut self,
        step: usize,
        tool: &str,
        params: Value,
    ) -> Value { ... }

    /// Call a tool expecting failure. Panics with full trace if it succeeds.
    pub async fn expect_err(
        &mut self,
        step: usize,
        tool: &str,
        params: Value,
    ) -> McpError { ... }

    /// Format the full trace for inclusion in assertion messages.
    pub fn trace_summary(&self) -> String { ... }

    /// Access the underlying harness (for mock event injection, etc.)
    pub fn harness(&self) -> &TestHarness { ... }
    pub fn mock(&self) -> &MockAddon { ... }
}

impl std::fmt::Display for DebugContext {
    /// Prints the trace in a readable format:
    /// ```
    /// Step 1: spatial_snapshot({detail: "standard"}) → OK (23ms)
    ///   entities: 5, budget.used: 320
    /// Step 2: spatial_inspect({node: "/Ghost"}) → ERR (2ms)
    ///   node_not_found: Node '/Ghost' not found
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { ... }
}
```

**Implementation Notes**:

Every `expect()` call wraps `harness.call_tool()` with timing and logging.
On failure, the panic message includes:

```
journey_investigate_wall_clip failed at step 4:
  Expected OK but got: node_not_found

Full trace:
  Step 1: spatial_snapshot({detail:"standard"}) → OK (15ms)
  Step 2: scene_tree({}) → OK (8ms)
  Step 3: spatial_query({query_type:"radius",...}) → OK (5ms)
  Step 4: spatial_action({action:"teleport",...}) → ERR (3ms)  ← FAILED HERE
    node_not_found: Node 'enemies/Scout' not found
```

This makes debugging journey test failures trivial — you see the full
conversation history at a glance.

**Acceptance Criteria**:
- [ ] `expect()` returns value on success
- [ ] `expect()` panics with full trace on failure
- [ ] `expect_err()` panics with full trace on unexpected success
- [ ] Trace includes step number, tool, params, result, and timing
- [ ] `trace_summary()` produces human-readable output

---

## Implementation Order

1. **`support/debug.rs`** — DebugContext wrapper (no dependencies)
2. **`support/world.rs`** — StatefulMockWorld (depends on fixtures, mock_addon)
3. **`support/mod.rs`** — add `pub mod debug; pub mod world;`
4. **`journeys.rs`** — all 5 journey tests (depends on all support modules)

Units 1-2 can be implemented in parallel. Unit 4 depends on both.

## Testing

### Unit Tests: `support/world.rs` (inline)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_entity_updates_position() {
        let world = StatefulMockWorld::from_scene(mock_scene_3d());
        world.move_entity("Player", vec![10.0, 0.0, 5.0]);
        // Verify via internal state access
    }

    #[test]
    fn set_state_updates_property() {
        let world = StatefulMockWorld::from_scene(mock_scene_3d());
        world.set_state("enemies/Scout", "health", json!(50));
    }

    #[test]
    fn handler_serves_current_state() {
        // Call handler with "get_snapshot_data", verify positions match
    }

    #[test]
    fn handler_teleport_mutates_world() {
        // Call handler with "execute_action" teleport, then "get_snapshot_data"
    }
}
```

### Integration Tests: `journeys.rs`

5 journey tests, each exercising 7-13 sequential tool calls. Run with:

```bash
cargo test -p spectator-server --features integration-tests journeys
```

Each test should complete in <2 seconds (mock TCP, no real Godot).

## Verification Checklist

```bash
# Build
cargo build -p spectator-server --features integration-tests

# Run journey tests
cargo test -p spectator-server --features integration-tests journeys -- --nocapture

# Run all integration tests (existing + new)
cargo test -p spectator-server --features integration-tests

# Lint
cargo clippy -p spectator-server --features integration-tests
```

## File Summary

| File | Purpose |
|------|---------|
| `tests/support/world.rs` | StatefulMockWorld — mutable game simulation |
| `tests/support/debug.rs` | DebugContext — trace-logging test wrapper |
| `tests/support/mod.rs` | Add module exports |
| `tests/journeys.rs` | 5 journey tests |
| `Cargo.toml` | Add `[[test]]` entry for journeys |
