# Refactor Plan: MCP Handler Consolidation

## Summary

The MCP tool handler layer (`crates/spectator-server/src/mcp/`) has accumulated
duplication across handler files as new tools were added. The main opportunities
are: duplicated default-value functions, repeated `query_addon → finalize`
boilerplate in clips handlers, a pattern violation in the config handler's
budget injection, duplicated frame-resolution logic in clip analysis handlers,
and identical `_error()` helpers across all GDScript director ops files.

This plan focuses on high-value, low-risk refactors that reduce duplication
without changing public APIs or wire format.

## Refactor Steps

### Step 1: Extract shared default functions into `mcp/defaults.rs`

**Priority**: High
**Risk**: Low
**Files**: `mcp/snapshot.rs`, `mcp/delta.rs`

**Current State**: `default_perspective()` and `default_radius()` are
copy-pasted identically in both `snapshot.rs:61-66` and `delta.rs:40-45`.
`default_detail()` exists only in `snapshot.rs:67-69`.

**Target State**: A single `mcp/defaults.rs` module exporting these functions,
imported by both `snapshot.rs` and `delta.rs`.

**Approach**:
1. Create `crates/spectator-server/src/mcp/defaults.rs`
2. Move the three default functions there
3. Update `#[serde(default = "...")]` paths in both param structs to reference
   the new module (serde `default` accepts a path string)
4. Remove the duplicate definitions

**Verification**:
- `cargo build -p spectator-server`
- `cargo test -p spectator-server` — all tests pass
- Grep for `default_perspective` and `default_radius` confirms single definition

### Step 2: Fix config handler budget injection (pattern violation)

**Priority**: High
**Risk**: Low
**Files**: `mcp/config.rs`

**Current State**: `handle_spatial_config()` (line 86-109) hardcodes
`"used": 50, "limit": 200` in the budget block and calls `serialize_response()`
instead of `finalize_response()`. This violates the MCP tool handler pattern
which requires all handlers to use `finalize_response()` with dynamically
computed budget values.

**Target State**: Use `finalize_response()` like every other handler, with
`resolve_budget(None, 200, config.token_hard_cap)` for the limit.

**Approach**:
1. Import `finalize_response` and `resolve_budget` in `config.rs`
2. Build the response JSON without the `budget` block
3. Call `finalize_response(&mut response, budget_limit, hard_cap)`

**Verification**:
- `cargo build -p spectator-server`
- `cargo test -p spectator-server`
- Manual inspection: config response now includes properly computed budget

### Step 3: Use `query_and_finalize` in clips action handlers

**Priority**: High
**Risk**: Low
**Files**: `mcp/clips.rs`

**Current State**: `handle_add_marker` (200-216), `handle_save` (218-230),
`handle_delete` (232-242), and `handle_markers` (244-254) each manually call
`query_addon()` then `finalize_response()` — the exact pattern that the
existing `query_and_finalize()` helper (189-198) was built to eliminate.

**Target State**: All four handlers use `query_and_finalize()` instead of
inline `query_addon + finalize_response`.

**Approach**:
1. Rewrite each handler to build its `json!(...)` params, then call
   `query_and_finalize(state, method, params, budget_limit, hard_cap)`
2. This is a ~4 line reduction per handler

**Verification**:
- `cargo test -p spectator-server`
- E2E tests: `theatre-deploy ~/dev/spectator/tests/godot-project && cargo test --workspace`

### Step 4: Extract `resolve_frame` helper in clip analysis handlers

**Priority**: Medium
**Risk**: Low
**Files**: `mcp/clips.rs`

**Current State**: The frame-resolution logic (resolve `at_frame` or
`at_time_ms` into a concrete frame number, with validation) is duplicated in
`handle_snapshot_at` (268-279), and similar `require_param! + validate_frame`
sequences appear in `handle_trajectory` (295-302) and `handle_query_range`
(327-334).

**Target State**: A `resolve_frame()` helper that handles the `at_frame` /
`at_time_ms` choice, and a `resolve_frame_range()` helper for the from/to
pattern.

**Approach**:
1. Add to `clips.rs`:
   ```rust
   fn resolve_frame(
       session: &ClipSession,
       at_frame: Option<u64>,
       at_time_ms: Option<u64>,
       action: &str,
   ) -> Result<u64, McpError> { ... }

   fn resolve_frame_range(
       session: &ClipSession,
       from: Option<u64>,
       to: Option<u64>,
       action: &str,
   ) -> Result<(u64, u64), McpError> { ... }
   ```
2. Replace inline logic in the three handlers

**Verification**:
- `cargo test -p spectator-server`
- E2E clip analysis tests pass

### Step 5: Consolidate `build_delta_json` conditional inserts

**Priority**: Medium
**Risk**: Low
**Files**: `mcp/delta.rs`

**Current State**: `build_delta_json()` (49-87) has five identical
`if !delta.field.is_empty() { map.insert(...) }` blocks that only differ in
field name.

**Target State**: A helper macro or loop that inserts non-empty fields:
```rust
fn insert_if_nonempty<T: Serialize>(map: &mut Map, key: &str, val: &[T]) {
    if !val.is_empty() {
        map.insert(key.into(), serde_json::to_value(val).unwrap_or_default());
    }
}
```

**Approach**:
1. Add `insert_if_nonempty` as a local helper in `delta.rs`
2. Replace the five conditional blocks with five one-line calls

**Verification**:
- `cargo test -p spectator-server` — delta tests pass
- JSON output unchanged (verify with existing snapshot comparison tests)

### Step 6: Extract shared `_error()` in GDScript director ops

**Priority**: High
**Risk**: Low
**Files**: `addons/director/ops/node_ops.gd`, `scene_ops.gd`,
`resource_ops.gd`, `tilemap_ops.gd`, `gridmap_ops.gd`

**Current State**: All five ops files define an identical static
`_error(message, operation, context) -> Dictionary` function.

**Target State**: A single shared utility (e.g., `addons/director/ops/ops_util.gd`)
defining `_error()`, imported by all ops files.

**Approach**:
1. Create `addons/director/ops/ops_util.gd` with `class_name OpsUtil`
2. Move `_error()` there as a static function
3. Replace `_error(...)` calls with `OpsUtil._error(...)` in all five files
4. Delete the local `_error` definitions

**Verification**:
- Open test project in Godot, run director operations
- All ops files parse without errors
- E2E director tests pass (if present)

### Step 7: Extract shared `_validate_node_type` in GDScript ops

**Priority**: Low
**Risk**: Low
**Files**: `addons/director/ops/tilemap_ops.gd`, `gridmap_ops.gd`

**Current State**: `_validate_tilemap_layer()` and `_validate_gridmap()` are
structurally identical — check `node is ExpectedType`, return error if not.

**Target State**: A generic `OpsUtil._validate_node_type(node, expected_class,
operation, context)` that uses `is_class()` for the check.

**Approach**:
1. Add to `ops_util.gd`:
   ```gdscript
   static func _validate_node_type(node: Node, expected: String,
       operation: String, context: Dictionary) -> Dictionary:
       if node.is_class(expected):
           return {"success": true}
       return _error("Node is %s, expected %s" % [node.get_class(), expected],
           operation, context)
   ```
2. Replace the two specific validators with calls to the generic one

**Verification**:
- Godot project loads without parse errors
- Tilemap and gridmap operations still validate correctly
