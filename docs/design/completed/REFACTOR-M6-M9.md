# Refactor Plan: MCP Tool Handlers & Recording (M6-M9)

## Summary

After M6-M9 implementation, the MCP tool handler layer has accumulated repetitive patterns: 8+ near-identical enum parsing functions, 15+ required-parameter validation boilerplate blocks, duplicated recording analysis setup across 4 handlers, and inconsistent budget finalization. This plan consolidates these into shared abstractions, ordered from highest impact to lowest, each step independently buildable and testable.

Zero pattern violations were found — the code follows all established patterns correctly. The refactoring targets duplication, not structural problems.

## Refactor Steps

### Step 1: Extract `parse_enum_param` generic helper

**Priority**: High
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/mod.rs`, `snapshot.rs`, `watch.rs`, `scene_tree.rs`, `inspect.rs`, `config.rs`

**Current State**: 8+ functions with identical structure — match string to enum variant, return `McpError::invalid_params` on mismatch:

```rust
// snapshot.rs:120-130
fn parse_detail(s: &str) -> Result<DetailLevel, McpError> {
    match s {
        "summary" => Ok(DetailLevel::Summary),
        "standard" => Ok(DetailLevel::Standard),
        "full" => Ok(DetailLevel::Full),
        _ => Err(McpError::invalid_params(format!("Invalid detail ..."), None)),
    }
}
// Same pattern in: parse_operator, parse_track, parse_action, parse_find_by,
// parse_include, parse_tree_include, parse_cluster_by, parse_bearing_format
```

**Target State**: A single generic helper in `mod.rs` that all parsers delegate to:

```rust
/// Parse a string parameter into an enum variant, returning McpError::invalid_params on mismatch.
fn parse_enum_param<T>(
    value: &str,
    field_name: &str,
    variants: &[(&str, T)],
) -> Result<T, McpError>
where
    T: Clone,
{
    for (name, variant) in variants {
        if *name == value {
            return Ok(variant.clone());
        }
    }
    let valid: Vec<&str> = variants.iter().map(|(n, _)| *n).collect();
    Err(McpError::invalid_params(
        format!("Invalid {field_name} '{value}'. Valid: {}", valid.join(", ")),
        None,
    ))
}
```

Each existing `parse_*` function becomes a one-liner:

```rust
pub fn parse_detail(s: &str) -> Result<DetailLevel, McpError> {
    parse_enum_param(s, "detail level", &[
        ("summary", DetailLevel::Summary),
        ("standard", DetailLevel::Standard),
        ("full", DetailLevel::Full),
    ])
}
```

**Approach**:
1. Add `parse_enum_param` to `mcp/mod.rs` (pub(super))
2. Refactor each `parse_*` function to use it, one file at a time
3. The `parse_include` / `parse_tree_include` array variants also get a `parse_enum_list` helper
4. Remove `parse_cluster_by` and `parse_bearing_format` in config.rs which use a different (serde-based) approach — unify to same pattern

**Verification**:
- `cargo test -p spectator-server` — existing parse tests pass
- `cargo clippy -p spectator-server` — no new warnings
- Grep confirms no remaining standalone match-to-McpError enum parsing blocks

---

### Step 2: Extract `require_param!` macro for required field validation

**Priority**: High
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/mod.rs`, `action.rs`, `recording.rs`, `query.rs`, `snapshot.rs`

**Current State**: 15+ instances of the same pattern:

```rust
let node = params.node.as_ref().ok_or_else(|| {
    McpError::invalid_params("'node' is required for teleport action", None)
})?;
```

Varies only by: field name, field type (`Option<T>` vs `Option<&T>`), and error message.

**Target State**: A macro in `mod.rs`:

```rust
/// Extract a required parameter, returning McpError::invalid_params if None.
macro_rules! require_param {
    ($expr:expr, $msg:expr) => {
        $expr.ok_or_else(|| McpError::invalid_params($msg.to_string(), None))?
    };
}
pub(super) use require_param;
```

Usage:

```rust
let node = require_param!(params.node.as_ref(), "'node' is required for teleport action");
let frames = require_param!(params.frames, "'frames' is required for advance_frames action");
```

**Approach**:
1. Add macro to `mcp/mod.rs`
2. Refactor `action.rs` first (highest density — 12 instances)
3. Then `recording.rs` (8 instances), `query.rs` (6 instances), `snapshot.rs` (2 instances)

**Verification**:
- `cargo test -p spectator-server` — all action/recording/query tests pass
- `cargo clippy -p spectator-server`
- Each file saves 2-3 lines per instance (~45 lines total)

---

### Step 3: Extract `RecordingSession` helper for analysis handlers

**Priority**: High
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/recording.rs`, `crates/spectator-server/src/recording_analysis.rs`

**Current State**: All 4 M8 analysis handlers (`handle_snapshot_at`, `handle_query_range`, `handle_diff_frames`, `handle_find_event`) repeat the same 4-line setup:

```rust
let storage_path = recording_analysis::resolve_storage_path(state).await?;
let recording_id = resolve_recording_id(params, &storage_path)?;
let db = recording_analysis::open_recording_db(&storage_path, &recording_id)?;
let meta = recording_analysis::read_recording_meta(&db)?;
```

And the same 3-line teardown:

```rust
if let Some(obj) = response.as_object_mut() {
    obj.insert("recording_context".into(), meta.to_context());
}
finalize_response(&mut response, budget_limit, hard_cap)
```

**Target State**: A `RecordingSession` struct in `recording_analysis.rs`:

```rust
pub struct RecordingSession {
    pub db: rusqlite::Connection,
    pub meta: RecordingMeta,
    pub storage_path: String,
    pub recording_id: String,
}

impl RecordingSession {
    pub async fn open(
        state: &Arc<Mutex<SessionState>>,
        recording_id: Option<&str>,
    ) -> Result<Self, McpError> {
        let storage_path = resolve_storage_path(state).await?;
        let recording_id = match recording_id {
            Some(id) => id.to_string(),
            None => most_recent_recording(&storage_path)
                .ok_or_else(|| McpError::invalid_params("No recordings found", None))?,
        };
        let db = open_recording_db(&storage_path, &recording_id)?;
        let meta = read_recording_meta(&db)?;
        Ok(Self { db, meta, storage_path, recording_id })
    }

    pub fn finalize(
        &self,
        response: &mut serde_json::Value,
        budget_limit: u32,
        hard_cap: u32,
    ) -> Result<String, McpError> {
        if let Some(obj) = response.as_object_mut() {
            obj.insert("recording_context".into(), self.meta.to_context());
        }
        super::finalize_response(response, budget_limit, hard_cap)
    }
}
```

Each handler shrinks from ~25 lines to ~12 lines.

**Approach**:
1. Add `RecordingSession` to `recording_analysis.rs`
2. Refactor each analysis handler in `recording.rs` to use it
3. Move `resolve_recording_id` logic into `RecordingSession::open`

**Verification**:
- `cargo test -p spectator-server` — recording tests pass
- All 4 analysis handlers produce identical output to before
- `resolve_recording_id` is no longer a standalone function

---

### Step 4: Unify budget finalization in `spatial_action`

**Priority**: Medium
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/mod.rs` (spatial_action handler)

**Current State**: `spatial_action` (mod.rs:343-346) manually calls `inject_budget` instead of `finalize_response`:

```rust
let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
let used = spectator_core::budget::estimate_tokens(json_bytes);
let action_budget = resolve_budget(None, 500, config.token_hard_cap);
inject_budget(&mut response, used, action_budget, config.token_hard_cap);
```

Every other handler uses `finalize_response`. This handler also has an inline `serde_json::to_value` at line 280 that duplicates the `serialize_response` helper.

**Target State**: Use `finalize_response` consistently:

```rust
let action_budget = resolve_budget(None, 500, config.token_hard_cap);
// ... (return_delta logic may mutate response) ...
finalize_response(&mut response, action_budget, config.token_hard_cap)
```

Move the `finalize_response` call to after the return_delta block so budget accounts for the delta payload too.

Also in `spatial_inspect` (mod.rs:280-282), replace the inline `.map_err` with `serialize_response` — but note this is `to_value` not `to_string`, so it's not a direct replacement. Leave as-is since `to_value` is a different operation.

**Approach**:
1. Move `finalize_response` to end of `spatial_action`, after return_delta
2. Remove manual `inject_budget` / `estimate_tokens` calls
3. Ensure response includes accurate budget with delta payload

**Verification**:
- `cargo test -p spectator-server` — action tests pass
- Budget values in action responses now accurately include delta payload size
- `grep -n "inject_budget" crates/spectator-server/src/mcp/` only shows the definition in mod.rs

---

### Step 5: Extract `query_and_deserialize` helper

**Priority**: Medium
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/mod.rs`, `recording.rs`, `query.rs`, `delta.rs`

**Current State**: 9+ instances of the same two-line pattern:

```rust
let data = query_addon(state, "method_name", serialize_params(&params)?).await?;
let response: T = deserialize_response(data)?;
```

**Target State**: A single generic helper:

```rust
async fn query_and_deserialize<P: Serialize, R: for<'de> Deserialize<'de>>(
    state: &Arc<Mutex<SessionState>>,
    method: &str,
    params: &P,
) -> Result<R, McpError> {
    let data = query_addon(state, method, serialize_params(params)?).await?;
    deserialize_response(data)
}
```

Usage:

```rust
let raw_data: SnapshotResponse = query_and_deserialize(&self.state, "get_snapshot_data", &query_params).await?;
```

**Approach**:
1. Add `query_and_deserialize` to `mcp/mod.rs` (pub(super))
2. Refactor callers one file at a time: mod.rs, delta.rs, query.rs
3. Leave recording.rs callers that pass raw `json!({})` params — those use `query_addon` directly with already-built JSON, which is fine

**Verification**:
- `cargo test -p spectator-server`
- Each refactored call site saves 2 lines
- `serialize_params` and `deserialize_response` remain available for edge cases

---

### Step 6: Deduplicate `build_standard_response` / `build_full_response`

**Priority**: Medium
**Risk**: Medium
**Files**: `crates/spectator-server/src/mcp/snapshot.rs`

**Current State**: `build_standard_response` (lines 359-419) and `build_full_response` (lines 421-481) share ~60% structure:
- Both create a `BudgetEnforcer`, add 200-byte overhead
- Both iterate entities, split static/dynamic
- Both call `build_output_entity` with budget enforcement
- Both build a pagination block on truncation
- Both build the same response shape with frame/timestamp/perspective/entities/budget

Differences:
- Standard: static entities as count+categories summary; `full=false` on output entities
- Full: static entities as individual `{path, class, pos}` nodes; `full=true` on output entities

**Target State**: Extract a common `build_snapshot_body` that takes a `SnapshotTier` enum:

```rust
enum SnapshotTier {
    Standard,
    Full,
}

fn build_snapshot_body(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
    config: &SessionConfig,
    tier: SnapshotTier,
) -> serde_json::Value { ... }
```

The tier controls: (a) whether static entities become counts or individual nodes, (b) whether `build_output_entity` gets `full=true`.

**Approach**:
1. Create `SnapshotTier` enum (not serialized — internal only)
2. Extract common loop into `build_snapshot_body`
3. Have `build_standard_response` and `build_full_response` call it
4. Keep function signatures unchanged for callers

**Verification**:
- `cargo test -p spectator-server` — snapshot tests pass
- Diff output of standard and full responses before/after to confirm identical JSON
- ~50 lines removed from snapshot.rs

---

### Step 7: Consolidate simple recording capture handlers

**Priority**: Low
**Risk**: Low
**Files**: `crates/spectator-server/src/mcp/recording.rs`

**Current State**: `handle_stop`, `handle_status`, `handle_list` are nearly identical 5-line functions:

```rust
async fn handle_stop(state: &..., budget_limit: u32, hard_cap: u32) -> Result<String, McpError> {
    let data = query_addon(state, "recording_stop", json!({})).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}
```

**Target State**: A helper that eliminates the three one-off functions:

```rust
async fn query_and_finalize(
    state: &Arc<Mutex<SessionState>>,
    method: &str,
    params: serde_json::Value,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let mut data = query_addon(state, method, params).await?;
    finalize_response(&mut data, budget_limit, hard_cap)
}
```

Then the dispatch becomes:

```rust
"stop" => query_and_finalize(state, "recording_stop", json!({}), budget_limit, hard_cap).await,
"status" => query_and_finalize(state, "recording_status", json!({}), budget_limit, hard_cap).await,
"list" => query_and_finalize(state, "recording_list", json!({}), budget_limit, hard_cap).await,
```

**Approach**:
1. Add `query_and_finalize` helper
2. Inline the three functions into match arms using the helper
3. Keep `handle_start`, `handle_delete`, `handle_markers`, `handle_add_marker` as separate functions (they have parameter-specific logic)

**Verification**:
- `cargo test -p spectator-server` — recording tests pass
- 3 functions (~15 lines) eliminated
- Match arms remain readable

---

## Not Refactored (Intentionally)

| Pattern | Reason |
|---|---|
| GDScript ProjectSettings loading | Only 3-4 instances, not worth abstracting |
| Watch action handler match arms | Each arm is small and distinct; extracting a trait adds complexity without reducing lines |
| Recording action handler dispatch | Simple match statement; trait-based dispatch is over-engineering |
| `to_entity_snapshot` vs `to_raw_entity` | Different downstream types with different fields; forcing unification would couple delta engine to clustering |
| 2D/3D position branching | Already localized to 2-3 spots per concern; a dimension processor abstraction would add indirection without reducing code |
| GDScript dock null-check patterns | Idiomatic GDScript; extracting would reduce readability |

## Implementation Order

```
Step 1 (parse_enum_param) ─┐
Step 2 (require_param!)  ──┼── independent, can be done in parallel
Step 4 (action budget)   ──┘
Step 3 (RecordingSession)  ── depends on nothing, but touches recording.rs
Step 5 (query_and_deserialize) ── independent
Step 6 (snapshot body)     ── independent, medium risk
Step 7 (recording helpers) ── independent, trivial
```

Steps 1, 2, and 4 can be applied first in any order. Steps 3, 5, 6, 7 are independent of each other but benefit from steps 1-2 being done first (less noise in diffs).

## Estimated Impact

| Metric | Before | After |
|---|---|---|
| Enum parse boilerplate | ~80 lines across 8 functions | ~30 lines (one-liner delegations) |
| Required param validation | ~55 lines across 15+ sites | ~15 lines (macro calls) |
| Recording analysis setup | ~28 lines (7 lines x 4 handlers) | ~4 lines (1 line x 4 handlers) |
| Snapshot response builders | ~120 lines (two near-identical functions) | ~70 lines (shared body + thin wrappers) |
| Total lines removed | ~120-150 lines of pure duplication | |
