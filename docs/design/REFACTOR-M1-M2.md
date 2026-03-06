# Refactor Plan: M1 & M2 Code Consolidation

## Summary

After M1 (spatial_snapshot) and M2 (spatial_inspect, scene_tree) were implemented
end-to-end, several patterns emerged that are duplicated across tool handlers,
the collector, and the query handler. This plan consolidates those patterns into
shared helpers, fixes a data consistency bug (static class lists out of sync),
and reduces the public API surface.

Each step is independent, buildable, and testable. Apply in order — later steps
may depend on earlier ones.

---

## Refactor Steps

### Step 1: Extract `vec_to_position3` / `vec_to_array3` helper in spectator-core

**Priority**: High
**Risk**: Low
**Files**: `crates/spectator-core/src/types.rs`, `crates/spectator-server/src/mcp/snapshot.rs`, `crates/spectator-server/src/mcp/inspect.rs`, `crates/spectator-server/src/mcp/mod.rs`

**Current State**: Four+ call sites manually index into `Vec<f64>` with
`.first().copied().unwrap_or(0.0)`, `.get(1)...`, `.get(2)...` to produce a
`Position3` or `[f64; 3]`.

- `snapshot.rs:224-228` (to_raw_entity position)
- `snapshot.rs:229-233` (to_raw_entity rotation)
- `snapshot.rs:234-238` (to_raw_entity velocity)
- `mod.rs:70-74` (entity position filtering)
- `inspect.rs:58-62` (node_position)
- `inspect.rs:63-67` (node_forward)
- `inspect.rs:75-78` (target position)

**Target State**: A single `pub fn vec_to_array3(v: &[f64]) -> [f64; 3]` in
`spectator-core/src/types.rs`, with an optional default parameter variant
`vec_to_array3_default(v: &[f64], default: f64) -> [f64; 3]` for the
`node_forward` case (defaults to -1.0 for z). All call sites use it.

**Approach**:
1. Add `vec_to_array3` to `spectator-core/src/types.rs`
2. Replace all manual indexing in snapshot.rs, inspect.rs, mod.rs
3. Re-export from `spectator_core` lib.rs if not already visible

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace`
- Grep for `.first().copied().unwrap_or` in server crate — should be zero hits

---

### Step 2: Extract `vector3_to_vec` helper in spectator-godot

**Priority**: High
**Risk**: Low
**Files**: `crates/spectator-godot/src/collector.rs`

**Current State**: 15+ instances of `vec![v.x as f64, v.y as f64, v.z as f64]`
scattered through collector.rs (lines 93-95, 111-113, 195-196, 227, 231,
333, 341, 358, 376, 469-476, 497, 503, 518, 731-732, 796).

**Target State**: A local helper `fn vec3_to_f64(v: Vector3) -> Vec<f64>` at the
top of collector.rs. All conversions use it. Negated-forward cases use
`vec3_to_f64(-fwd)` or a `vec3_neg_to_f64` variant.

**Approach**:
1. Add `fn vec3_to_f64(v: Vector3) -> Vec<f64>` as a free function in collector.rs
2. Replace all `vec![v.x as f64, v.y as f64, v.z as f64]` patterns
3. For negated forward vectors: pass `-fwd` to the helper (Vector3 supports Neg)

**Verification**:
- `cargo build -p spectator-godot`
- Grep for `as f64, v.y as f64` in collector.rs — should be zero hits

---

### Step 3: Consolidate static class lists into spectator-core

**Priority**: High (bug fix — lists are out of sync)
**Risk**: Low
**Files**: `crates/spectator-core/src/types.rs` (or new `static_classes.rs`), `crates/spectator-server/src/mcp/snapshot.rs`, `crates/spectator-godot/src/collector.rs`

**Current State**: Two independent static class definitions that disagree:
- `collector.rs:15-32`: `STATIC_CLASSES` array includes `MeshInstance3D`
- `snapshot.rs:483-502`: `is_static_class()` uses `matches!` macro, missing `MeshInstance3D`

This means the addon treats MeshInstance3D as static but the server doesn't,
leading to inconsistent entity categorization.

**Target State**: Single `STATIC_CLASSES` list and `is_static_class()` /
`classify_static_category()` functions in `spectator-core`. Both crates import
from there.

Note: spectator-godot depends on spectator-protocol, NOT spectator-core. So the
shared list must go in spectator-protocol (which both depend on) or we accept
that spectator-godot duplicates a simple `contains` check. Best approach: put the
canonical list in spectator-protocol since it's the shared wire-format crate both
already depend on.

**Approach**:
1. Add `static_classes` module to spectator-protocol with the list and helpers
2. Update spectator-server/snapshot.rs to use `spectator_protocol::static_classes::*`
3. Update spectator-godot/collector.rs to use `spectator_protocol::static_classes::STATIC_CLASSES`
4. Ensure the list is consistent (include MeshInstance3D or not — decide once)
5. Delete the duplicated definitions

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`
- Grep for `STATIC_CLASSES` — should only appear in spectator-protocol
- Grep for `is_static_class` — should only appear in spectator-protocol + call sites

---

### Step 4: Extract MCP serde helpers in spectator-server

**Priority**: High
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/mod.rs` (new helpers or new `mcp/helpers.rs`)

**Current State**: 8+ identical serde error-wrapping blocks across the three
tool handlers in mod.rs:
- `to_value(&params).map_err(|e| McpError::internal_error(format!("Param serialization error: {e}"), None))`
- `from_value(data).map_err(|e| McpError::internal_error(format!("Response deserialization error: {e}"), None))`
- `to_string(&response).map_err(|e| McpError::internal_error(format!("Response serialization error: {e}"), None))`

**Target State**: Three helper functions (can live at the top of mod.rs or in a
small helpers module):

```rust
fn serialize_params<T: Serialize>(params: &T) -> Result<serde_json::Value, McpError>;
fn deserialize_response<T: DeserializeOwned>(data: serde_json::Value) -> Result<T, McpError>;
fn serialize_response<T: Serialize>(response: &T) -> Result<String, McpError>;
```

**Approach**:
1. Add the three helpers at the top of mod.rs (below imports)
2. Replace all inline `.map_err(...)` blocks in the three tool handlers
3. Consider also extracting a `query_and_deserialize<T>` that combines
   query_addon + deserialize_response (used identically by snapshot and inspect)

**Verification**:
- `cargo build -p spectator-server`
- `cargo test -p spectator-server`
- Grep for `Param serialization error` — should appear only in the helper

---

### Step 5: Extract query handler serde helpers in spectator-godot

**Priority**: Medium
**Risk**: Low
**Files**: `crates/spectator-godot/src/query_handler.rs`

**Current State**: Four handler functions repeat identical `QueryError` wrapping
for deserialization (lines 46, 70, 90) and serialization (lines 52, 60, 80):

```rust
serde_json::from_value(params).map_err(|e| QueryError {
    code: "invalid_params".to_string(),
    message: format!("Invalid params: {e}"),
})?;
```

**Target State**: Two local helpers:

```rust
fn parse_params<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, QueryError>;
fn to_json_value<T: Serialize>(data: &T) -> Result<serde_json::Value, QueryError>;
```

**Approach**:
1. Add helpers above the handler functions
2. Replace all inline constructions
3. Each handler becomes 2-3 lines shorter

**Verification**:
- `cargo build -p spectator-godot`
- Grep for `"invalid_params".to_string()` — should appear only in the helper

---

### Step 6: Extract budget injection helper

**Priority**: Medium
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/mod.rs`

**Current State**: spatial_inspect (lines 177-186) and scene_tree (lines 220-229)
both manually inject a budget JSON block into a `serde_json::Value::Object`:

```rust
if let serde_json::Value::Object(ref mut map) = response {
    map.insert("budget".to_string(), serde_json::json!({
        "used": used, "limit": limit, "hard_cap": SnapshotBudgetDefaults::HARD_CAP,
    }));
}
```

(spatial_snapshot uses BudgetEnforcer::report() inline in json! macros, which is
fine — it's a different pattern.)

**Target State**: A helper function:

```rust
fn inject_budget(response: &mut serde_json::Value, used: u32, limit: u32);
```

**Approach**:
1. Add helper in mod.rs (or helpers module from step 4)
2. Replace the two manual injection blocks

**Verification**:
- `cargo build -p spectator-server`
- `cargo test -p spectator-server`

---

### Step 7: Extract perspective block builder in snapshot.rs

**Priority**: Medium
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/snapshot.rs`

**Current State**: The perspective JSON block is built identically in
build_summary_response (line 288), build_standard_response (lines 341, 358),
and build_full_response (lines 410, 427):

```rust
"perspective": {
    "position": raw.perspective.position,
    "facing": perspective.facing,
    "facing_deg": perspective.facing_deg,
}
```

**Target State**: A helper function:

```rust
fn perspective_json(raw: &PerspectiveData, persp: &Perspective) -> serde_json::Value;
```

**Approach**:
1. Add helper in snapshot.rs
2. Replace all inline perspective blocks with the helper call

**Verification**:
- `cargo build -p spectator-server`
- `cargo test -p spectator-server`
- Grep for `"facing_deg"` in snapshot.rs — should appear only in the helper

---

### Step 8: Reduce snapshot.rs public API surface

**Priority**: Low
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/snapshot.rs`

**Current State**: Several functions are `pub` but only used within the mcp
module:
- `default_perspective()`, `default_radius()`, `default_detail()` (line 58-66) —
  only used by serde defaults on SpatialSnapshotParams
- `is_static_class()`, `classify_static_category()` (lines 483, 504) — after
  step 3, these move out entirely

**Target State**: Default functions are `fn` (private). After step 3, static
class functions are removed from this file.

**Approach**:
1. Remove `pub` from the three default functions (they're already private
   based on serde usage, verify no external callers)
2. Confirm is_static_class/classify_static_category have been moved (step 3)

**Verification**:
- `cargo build -p spectator-server`
- `cargo clippy --workspace` — no dead_code warnings

---

### Step 9: Unify TCP nonblocking guard in tcp_server.rs

**Priority**: Low
**Risk**: Medium (touches I/O code paths)
**Files**: `crates/spectator-godot/src/tcp_server.rs`

**Current State**: Three methods (`send_handshake`, `try_read`, `send_response`)
each manually toggle `set_nonblocking(false)` before I/O and
`set_nonblocking(true)` after, with error paths that can skip the restore.

**Target State**: A scoped helper that restores nonblocking mode on drop:

```rust
fn with_blocking_io<F, R>(stream: &TcpStream, f: F) -> R
where F: FnOnce(&TcpStream) -> R {
    stream.set_nonblocking(false).ok();
    let result = f(stream);
    stream.set_nonblocking(true).ok();
    result
}
```

Or a RAII guard struct. The key benefit is correctness — early returns from error
paths currently skip the restore in some cases.

**Approach**:
1. Add a `BlockingGuard` struct or closure-based helper
2. Refactor send_handshake, try_read, send_response to use it
3. Test manually with a running Godot scene (no automated test for TCP I/O)

**Verification**:
- `cargo build -p spectator-godot`
- Manual test: connect MCP server to running Godot game, verify handshake + queries work
- Grep for `set_nonblocking` — should appear only in the guard

---

## Excluded / Deferred

These were identified but not worth refactoring now:

- **Enum parsing macro/trait**: The 5 parse functions (parse_detail,
  parse_include, parse_action, parse_find_by, parse_tree_include) follow the
  same pattern but each has different enum types, valid values, and error
  messages. A macro would save ~3 lines per function but add indirection. Not
  worth it for 5 instances. Reconsider if M3-M9 add more tools.

- **Spatial logic in collector.rs**: The structural agent flagged
  `collect_nearby_recursive` and `collect_containing_areas` in spectator-godot
  as "spatial reasoning that should be in spectator-core." However, these
  functions traverse the live Godot scene tree (using `Gd<Node>`, `try_cast`,
  etc.) and cannot run outside the Godot process. The addon collects raw spatial
  data; the server computes bearings and relative positions. This split is
  correct.

- **Inconsistent budget approaches**: snapshot uses BudgetEnforcer (streaming),
  inspect/scene_tree compute post-hoc. These are genuinely different use cases
  (streaming truncation vs. reporting). No unification needed.

- **Response builder consistency**: snapshot.rs exposes separate builder
  functions per detail level; inspect/scene_tree build inline in mod.rs. The
  snapshot builders are complex (500 lines with pagination, clustering, static
  summaries) and warrant separation. The others are simple enough to stay inline.
  Not worth forcing consistency.
