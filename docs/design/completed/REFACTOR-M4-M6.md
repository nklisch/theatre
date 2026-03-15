# Refactor Plan: M4–M6 & Overall Repo Consolidation

## Summary

After M4 (deltas & watches), M5 (configuration), and M6 (editor dock) were
implemented, the codebase has grown to 9 MCP tool handlers with significant
pattern repetition across them. The previous REFACTOR-M1-M2 addressed early
duplication (vec conversions, static class lists, serde helpers, budget
injection). This plan targets patterns that emerged or expanded in M4–M6:
duplicated delta-building logic, watch condition formatting, config-clone
boilerplate, budget+serialize epilogues, perspective construction, and
structural issues like oversized modules and dead fields.

Each step is independent, buildable, and testable. Apply in order — later steps
may depend on earlier ones.

---

## Refactor Steps

### Step 1: Extract `get_config` helper for session config access

**Priority**: High
**Risk**: Low
**Files**: `crates/stage-server/src/tcp.rs`, `crates/stage-server/src/mcp/mod.rs`, `crates/stage-server/src/mcp/delta.rs`, `crates/stage-server/src/mcp/query.rs`, `crates/stage-server/src/mcp/watch.rs`, `crates/stage-server/src/mcp/config.rs`

**Current State**: 8 call sites repeat the identical 3-line block:

```rust
let config = {
    let s = self.state.lock().await;
    s.config.clone()
};
```

Found in: `mod.rs:98-101` (snapshot), `mod.rs:221-224` (inspect),
`mod.rs:275-278` (scene_tree), `mod.rs:303-306` (action), `delta.rs:78-81`,
`query.rs:249-252`, `watch.rs:86-89`, `config.rs:75-78`.

**Target State**: A single async helper:

```rust
// In tcp.rs or a new helpers module
pub async fn get_config(state: &Arc<Mutex<SessionState>>) -> SessionConfig {
    state.lock().await.config.clone()
}
```

All 8 call sites reduced to `let config = get_config(&self.state).await;` or
`let config = get_config(state).await;`.

**Approach**:
1. Add `get_config()` to `tcp.rs` (next to `query_addon`)
2. Replace all 8 call sites
3. For handlers on `&self` (mod.rs), call `get_config(&self.state)`
4. For free functions (delta, query, watch, config), pass `state` as they
   already do

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`
- Grep for `s.config.clone()` in server crate — should appear only in the
  helper (or `toml_to_session_config`)

---

### Step 2: Extract `finalize_response` budget+serialize epilogue

**Priority**: High
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/mod.rs`, `crates/stage-server/src/mcp/delta.rs`, `crates/stage-server/src/mcp/watch.rs`, `crates/stage-server/src/mcp/query.rs`

**Current State**: 9+ call sites repeat the identical 4-line epilogue:

```rust
let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
let used = estimate_tokens(json_bytes);
inject_budget(&mut response, used, budget_limit, hard_cap);
serialize_response(&response)
```

Found in: `mod.rs:252-257` (inspect), `mod.rs:280-287` (scene_tree),
`mod.rs:318-321` (action), `delta.rs:209-213`, `query.rs:357-361`,
`watch.rs:145-149` (add), `watch.rs:166-170` (remove), `watch.rs:209-213`
(list), `watch.rs:226-230` (clear).

**Target State**: A single helper in `mod.rs`:

```rust
fn finalize_response(
    response: &mut serde_json::Value,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
    let used = estimate_tokens(json_bytes);
    inject_budget(response, used, budget_limit, hard_cap);
    serialize_response(&response)
}
```

Each call site becomes one line: `finalize_response(&mut response, limit, cap)`.

**Approach**:
1. Add `finalize_response` in `mod.rs` alongside existing helpers
2. Re-export from mod.rs so submodules can use it via `super::finalize_response`
3. Replace all 9 call sites

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`
- Grep for `estimate_tokens` in mcp/ — should appear only in the helper

---

### Step 3: Extract `build_delta_json` for delta map construction

**Priority**: High
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/delta.rs`, `crates/stage-server/src/mcp/mod.rs`

**Current State**: The delta JSON map construction (conditionally inserting
moved, state_changed, entered, exited, watch_triggers) is duplicated between:
- `delta.rs:146-205` (spatial_delta handler)
- `mod.rs:372-410` (spatial_action return_delta inline)

Both build a `serde_json::Value::Object` and conditionally insert the same 5
categories with identical patterns.

**Target State**: A shared function in `delta.rs`:

```rust
pub fn build_delta_json(
    delta: &DeltaResult,
    watch_triggers: &[WatchTrigger],
) -> serde_json::Value
```

The spatial_delta handler calls this, adds signals_emitted separately (only
delta has those). The spatial_action return_delta block calls it directly.

**Approach**:
1. Extract the shared part into `build_delta_json` in `delta.rs`
2. Update `handle_spatial_delta` to call it, then append signals_emitted
3. Update `spatial_action` return_delta block in `mod.rs` to call it
4. Remove ~30 lines of duplicate code from `mod.rs`

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`
- Grep for `"moved".into()` in mcp/ — should appear only in `delta.rs`

---

### Step 4: Extract `format_conditions` for watch condition descriptions

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/watch.rs`

**Current State**: The condition-to-string formatting logic is duplicated
between the "add" action (lines 120-136) and "list" action (lines 179-194):

```rust
let conditions_desc = if w.conditions.is_empty() {
    "none".to_string()
} else {
    w.conditions.iter()
        .map(|c| {
            let val = c.value.as_ref().map(|v| v.to_string()).unwrap_or_default();
            format!("{} {:?} {}", c.property, c.operator, val)
        })
        .collect::<Vec<_>>()
        .join(", ")
};
```

**Target State**: A single helper:

```rust
fn format_conditions(conditions: &[WatchCondition]) -> String {
    if conditions.is_empty() { ... } else { ... }
}
```

Both call sites become `let conditions_desc = format_conditions(&watch.conditions);`.

**Approach**:
1. Add `format_conditions` as a free function in `watch.rs`
2. Replace both blocks

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server`
- Grep for `"none".to_string()` in watch.rs — should appear only in the helper

---

### Step 5: Extract `build_perspective_for_query` in query.rs

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/query.rs`

**Current State**: The perspective-from-optional-forward pattern appears 4
times in query.rs (lines 113-115, 142-144, 177-179, 182-184):

```rust
let perspective = from_forward
    .map(|fwd| perspective_from_forward(from_pos, fwd))
    .unwrap_or_else(|| perspective_from_yaw(from_pos, 0.0));
```

**Target State**: A local helper:

```rust
fn build_perspective_for_query(pos: Position3, forward: Option<[f64; 3]>) -> Perspective {
    forward
        .map(|fwd| perspective_from_forward(pos, fwd))
        .unwrap_or_else(|| perspective_from_yaw(pos, 0.0))
}
```

**Approach**:
1. Add helper at top of `query.rs`
2. Replace all 4 call sites

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server`
- Grep for `perspective_from_yaw.*0\.0` in query.rs — should appear only in
  the helper

---

### Step 6: Extract `build_query_entry` for nearest/radius result formatting

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/query.rs`

**Current State**: `build_nearest_response` (lines 117-128) and
`build_radius_response` (lines 146-157) both contain identical JSON entry
mapping:

```rust
.map(|r| {
    let rel = bearing::relative_position(&perspective, r.position, false);
    serde_json::json!({
        "path": r.path,
        "dist": (r.distance * 10.0).round() / 10.0,
        "bearing": rel.bearing,
        "class": r.class,
    })
})
```

**Target State**: A shared helper:

```rust
fn query_result_entry(r: &NearestResult, perspective: &Perspective) -> serde_json::Value
```

**Approach**:
1. Add helper in `query.rs`
2. Replace both mapping closures

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server`

---

### Step 7: Extract `is_entity_static` helper in snapshot.rs

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/snapshot.rs`

**Current State**: The static classification check appears 3 times:
- `snapshot.rs:268` (to_raw_entity)
- `snapshot.rs:351` (build_standard_response)
- `snapshot.rs:413` (build_full_response)

All use: `config.matches_static_pattern(&e.path) || is_static_class(&e.class)`

**Target State**: A local helper:

```rust
fn is_entity_static(entity: &EntityData, config: &SessionConfig) -> bool {
    config.matches_static_pattern(&entity.path) || is_static_class(&entity.class)
}
```

**Approach**:
1. Add helper in `snapshot.rs`
2. Replace all 3 call sites

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server`
- Grep for `is_static_class` in snapshot.rs — should appear only in the helper

---

### Step 8: Wire `since_frame` parameter in spatial_delta

**Priority**: Medium (bug fix — documented param is ignored)
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/delta.rs`

**Current State**: `SpatialDeltaParams.since_frame` (line 22) is declared,
documented in the MCP tool description ("Use since_frame to diff against a
specific frame"), but never read in `handle_spatial_delta`. The delta engine
always diffs against the last stored snapshot.

**Target State**: Either:
- (A) Wire it: if `since_frame` is provided, the delta engine diffs against
  that frame's stored snapshot (requires DeltaEngine to keep a frame history
  — this is out of scope for a simple refactor).
- (B) Remove it: delete the field and remove the mention from the tool
  description.

**Chosen: Option B** — remove the dead field. The delta engine's current
design stores only the last snapshot. Frame history would be a feature
addition, not a refactor.

**Approach**:
1. Remove `since_frame` field from `SpatialDeltaParams`
2. Remove "Use since_frame to diff against a specific frame." from the tool
   description in `mod.rs:449`
3. Update the test in `delta.rs:226`

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`
- Grep for `since_frame` — should have zero hits

---

### Step 9: Remove dead config fields

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/config.rs`

**Current State**: Several parsed TOML config sections are never used:
- `RecordingConfig` (lines 33-38): `storage_path`, `max_frames`,
  `capture_interval` — parsed but never read
- `DisplayConfig` (lines 40-43): `show_agent_notifications`,
  `show_recording_indicator` — parsed but never read

These are M7 (recording) and M6 features that exist in Godot Project Settings
(read by GDScript) but don't need to exist in the server's TOML parser yet.

**Target State**: Remove `RecordingConfig` and `DisplayConfig` structs and
their fields from `StageToml`. Keep the `[recording]` and `[display]`
TOML table names as comments documenting they'll be added when needed.

**Approach**:
1. Remove `RecordingConfig` struct and `recording` field from `StageToml`
2. Remove `DisplayConfig` struct and `display` field from `StageToml`
3. Add comment: `// [recording] and [display] sections parsed when M7 lands`

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace` (including the TOML parsing tests)

---

### Step 10: Remove dead `cursor` field from snapshot params

**Priority**: Low
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/snapshot.rs`

**Current State**: `SpatialSnapshotParams.cursor` (line 52) is declared but
never read. Pagination cursors are generated in responses but the consumer
side (using a cursor to resume) is not implemented.

**Target State**: Remove the field. Pagination can be re-added when the
feature is implemented.

**Approach**:
1. Remove `cursor` field from `SpatialSnapshotParams`

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`

---

### Step 11: Reduce visibility of internal cluster functions

**Priority**: Low
**Risk**: Low
**Files**: `crates/stage-core/src/cluster.rs`

**Current State**: Individual clustering strategy functions are `pub` but
only called from `cluster_entities()`:
- `cluster_by_group()` (line 51)
- `cluster_by_class()` (line 144)
- `cluster_by_proximity()` (line 180)
- `generate_cluster_summary()` (line 297)

The `cluster_none()` function is already private (`fn`). External callers
should use `cluster_entities()` which dispatches by strategy.

**Target State**: Change `pub fn` to `pub(crate) fn` for the four functions.
The public API surface is just `cluster_entities()` and the `Cluster` /
`ClusterStrategy` types.

**Approach**:
1. Change `pub fn cluster_by_group` → `pub(crate) fn cluster_by_group`
2. Same for `cluster_by_class`, `cluster_by_proximity`,
   `generate_cluster_summary`
3. Check that no code outside stage-core calls these directly

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`

---

### Step 12: Reduce visibility of `values_equal` and `evaluate_condition`

**Priority**: Low
**Risk**: Low
**Files**: `crates/stage-core/src/delta.rs`, `crates/stage-core/src/watch.rs`

**Current State**:
- `values_equal()` in `delta.rs:295` is `pub` but only called from
  `detect_state_changes()` in the same file and from `evaluate_condition()`
  in `watch.rs`.
- `evaluate_condition()` in `watch.rs:200` is a free `fn` (already private,
  not pub). No change needed there.

Since both call sites are within `stage-core`, `values_equal` should be
`pub(crate)`.

**Target State**: Change `pub fn values_equal` → `pub(crate) fn values_equal`.

**Approach**:
1. Change visibility
2. Verify no external crate calls it

**Verification**:
- `cargo build --workspace`

---

### Step 13: Remove dead `HandshakeInfo` fields (or use them)

**Priority**: Low
**Risk**: Low
**Files**: `crates/stage-server/src/tcp.rs`

**Current State**: `HandshakeInfo` (lines 73-81) stores 5 fields from the
addon handshake but none are ever read after being stored:
- `stage_version`
- `godot_version`
- `scene_dimensions`
- `physics_ticks_per_sec`
- `project_name`

These are potentially useful (e.g., `scene_dimensions` for 2D/3D adaptation)
but currently dead.

**Target State**: Keep the struct (it will be needed for 2D/3D adaptation and
spatial_config reporting), but add `#[allow(dead_code)]` with a TODO comment:

```rust
/// Information received from the addon during handshake.
/// TODO: Use scene_dimensions for 2D/3D spatial index selection,
///       expose via spatial_config "view current" output.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HandshakeInfo { ... }
```

**Approach**:
1. Add `#[allow(dead_code)]` and TODO comment
2. No functional changes

**Verification**:
- `cargo build --workspace` (no warnings for these fields)

---

## Excluded / Deferred

These were identified but not worth refactoring now:

- **Parse function macro/trait**: The 8+ `parse_*` functions
  (parse_detail, parse_include, parse_action, parse_find_by,
  parse_tree_include, parse_operator, parse_track, parse_cluster_by,
  parse_bearing_format) follow the same pattern but each has different enum
  types, valid values, and error messages. A macro would add indirection for
  ~3 lines saved per function. The existing M1-M2 refactor plan noted the
  same thing and deferred it. Still not worth it at 8 instances. Reconsider
  if M7-M9 add more.

- **Tool handler scaffold pattern**: All 8 tool handlers follow a similar
  structure (get config → query addon → deserialize → process → budget →
  serialize). A generic handler builder would save structural boilerplate
  but each handler's processing step is unique enough that the abstraction
  would be complex. The simpler helpers in Steps 1-3 address the repeated
  parts without adding framework complexity.

- **collector.rs split (1358 lines)**: The stage-godot collector is
  large but cohesive — it's the single scene tree interface and all methods
  need `&self` access to the base Node. Splitting into submodules would
  require either re-exporting through the main struct or passing `&self`
  around, adding complexity. The GDExtension pattern naturally concentrates
  API surface. Revisit if the file exceeds 2000 lines.

- **query.rs type organization (667 lines)**: stage-protocol's query.rs
  contains 35+ types in a flat namespace. While large, these are all wire
  format types that belong together. Splitting by concern (snapshot_types,
  action_types) would add import complexity for minimal benefit. All types
  are simple structs/enums with serde derives.

- **Required parameter extraction pattern**: The 15+ `ok_or_else` calls in
  `action.rs` for required parameters are each unique (different field name,
  different error message). A generic helper would save ~1 line per call at
  the cost of readability. Not worth it.

- **`serde_json::to_value(...).unwrap_or_default()`**: Appears 14+ times
  but in genuinely different contexts (delta categories, entity data, etc.).
  The pattern is a single expression — wrapping it would just add
  indirection.
