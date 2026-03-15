# Refactor Plan: Full Workspace

## Summary

Analysis of all four crates identified 8 refactoring opportunities across three
categories: duplicate logic (19x SQLite error boilerplate, cross-crate serde
helpers), missing abstractions (budget resolution, query response builders), and
pattern violations (contract violation in watch remove response, unwrap in
library helpers). Each step below is self-contained, buildable, and testable
independently.

---

## Refactor Steps

### Step 1: Extract SQLite error helper in clip_analysis

**Priority**: High
**Risk**: Low
**Files**: `crates/stage-server/src/clip_analysis.rs`

**Current State**: 19 occurrences of
`.map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))`
scattered across clip analysis functions.

**Target State**: A single `fn sqlite_err(e: impl std::fmt::Display) -> McpError`
helper at the top of the file, called from all 19 sites.

**Approach**:
1. Add the helper function near the existing `open_clip_db` function
2. Replace all 19 `.map_err(...)` closures with `.map_err(sqlite_err)?`
3. For the two `match` arms that use this pattern (lines ~1300, ~1330), use
   `Err(e) => Err(sqlite_err(e))`

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server` (all clip analysis tests pass)
- `cargo clippy -p stage-server`
- Grep confirms zero remaining inline `"SQLite error"` format strings

---

### Step 2: Fix contract violation — watch remove response must echo watch_id

**Priority**: High
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/watch.rs`

**Current State** (line 151):
```rust
let mut response = serde_json::json!({
    "result": if removed { "ok" } else { "not_found" },
    "removed": if removed { 1 } else { 0 },
});
```
Violates contracts.md: "Delete/remove responses must echo the id, not a boolean."

**Target State**:
```rust
let mut response = serde_json::json!({
    "result": if removed { "ok" } else { "not_found" },
    "watch_id": watch_id,
});
```

**Approach**: Replace the `"removed"` field with `"watch_id"` echoing the input.

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server`
- Manual review: response now echoes `watch_id` per contract rules

---

### Step 3: Replace unwrap_or_default in finalize_response and insert_if_nonempty

**Priority**: High
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/mod.rs`

**Current State**:
- Line 123: `serde_json::to_value(val).unwrap_or_default()` — silently produces
  `Null` on serialization failure
- Line 173: `serde_json::to_vec(response).unwrap_or_default().len()` — silently
  produces 0 byte count on failure

These violate error-layering: library helpers should not silently swallow errors.

**Target State**:
- `insert_if_nonempty` returns `Result<(), McpError>` or propagates via `?`
  (callers already return `Result<_, McpError>`)
- `finalize_response` uses `?` on the `to_vec` call

**Approach**:
1. Change `insert_if_nonempty` signature to return `Result<(), McpError>`,
   map the serde error to `McpError::internal_error`
2. Update all callers of `insert_if_nonempty` to use `?`
3. In `finalize_response`, replace `unwrap_or_default()` with
   `.map_err(|e| McpError::internal_error(...))?`

**Verification**:
- `cargo build -p stage-server` (callers updated)
- `cargo test --workspace`
- `cargo clippy --workspace`

---

### Step 4: Extract shared serde helpers to stage-protocol

**Priority**: Medium
**Risk**: Low
**Files**:
- `crates/stage-protocol/src/lib.rs` (new `mcp_helpers` module)
- `crates/stage-server/src/mcp/mod.rs` (remove duplicates, re-import)
- `crates/director/src/mcp/mod.rs` (remove duplicates, re-import)

**Current State**: `serialize_params` and `serialize_response` are defined
identically in both `stage-server/src/mcp/mod.rs:34-49` and
`director/src/mcp/mod.rs:61-69`.

**Target State**: Both functions live in `stage-protocol::mcp_helpers` and
are imported by both crates.

**Approach**:
1. Add `rmcp` as an optional dependency of `stage-protocol` behind an
   `mcp` feature flag (both server and director already depend on rmcp)
2. Create `crates/stage-protocol/src/mcp_helpers.rs` with the three
   functions: `serialize_params`, `deserialize_response`, `serialize_response`
3. In both consuming crates, replace local definitions with
   `use stage_protocol::mcp_helpers::*`

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`
- Grep confirms no remaining duplicate definitions

---

### Step 5: Extract BudgetContext helper for config + budget resolution

**Priority**: Medium
**Risk**: Low
**Files**:
- `crates/stage-server/src/mcp/mod.rs` (add helper)
- 8+ MCP handler files (simplify repeated pattern)

**Current State**: Every handler that needs budget does:
```rust
let config = get_config(&self.state).await;
let budget_limit = resolve_budget(params.token_budget, TIER_DEFAULT, config.token_hard_cap);
// ... later ...
finalize_response(&mut response, budget_limit, config.token_hard_cap)
```
This 3-line sequence appears in 8+ handlers with only `TIER_DEFAULT` varying.

**Target State**: A `BudgetContext` struct + async constructor:
```rust
pub(crate) struct BudgetContext { pub limit: u32, pub hard_cap: u32 }

pub(crate) async fn budget_context(
    state: &Arc<Mutex<SessionState>>,
    token_budget: Option<u32>,
    default: u32,
) -> BudgetContext { ... }
```
Handlers call `let bc = budget_context(&self.state, params.token_budget, 800).await;`
then `finalize_response(&mut response, bc.limit, bc.hard_cap)`.

**Approach**:
1. Add `BudgetContext` struct and `budget_context` function to `mcp/mod.rs`
2. Update each handler one at a time, verifying build between each
3. Remove now-unused direct `get_config` + `resolve_budget` imports where
   they were only used for budget

**Verification**:
- `cargo build -p stage-server`
- `cargo test --workspace`
- Each handler is shorter by 2 lines; the budget resolution logic is in one place

---

### Step 6: Unify nearest/radius query response builders

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/query.rs`

**Current State**: `build_nearest_response` (lines 119-134) and
`build_radius_response` (lines 136-153) are nearly identical — both build a
perspective, map results through `query_result_entry`, and wrap in a JSON
envelope. The only difference is the envelope fields.

**Target State**: One `build_list_query_response` function:
```rust
fn build_list_query_response(
    query_type: &str,
    results: &[NearestResult],
    from_pos: Position3,
    from_forward: Option<[f64; 3]>,
    extra: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value
```

**Approach**:
1. Create the unified function
2. Rewrite `build_nearest_response` and `build_radius_response` as thin
   wrappers (or inline their callers directly)
3. Update call sites

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server`
- Existing query integration tests still pass

---

### Step 7: Extract entity-to-snapshot conversion to a shared module

**Priority**: Low
**Risk**: Low
**Files**:
- `crates/stage-server/src/mcp/snapshot.rs` (`to_entity_snapshot`)
- `crates/stage-server/src/mcp/mod.rs` (`update_spatial_state` calls it)
- `crates/stage-server/src/mcp/delta.rs` (imports it)

**Current State**: `to_entity_snapshot` lives in `snapshot.rs` but is used by
both snapshot and delta modules. It converts protocol `EntityData` to core
`EntitySnapshot`.

**Target State**: Move to a small `conversions.rs` module under
`crates/stage-server/src/mcp/` that both snapshot and delta import from,
making the dependency explicit rather than going through `snapshot::`.

**Approach**:
1. Create `crates/stage-server/src/mcp/conversions.rs`
2. Move `to_entity_snapshot` (and possibly `to_raw_entity` if delta also needs it)
3. Update imports in `snapshot.rs`, `delta.rs`, and `mod.rs`

**Verification**:
- `cargo build -p stage-server`
- `cargo test --workspace`

---

### Step 8: Consolidate director error conversions with a macro

**Priority**: Low
**Risk**: Low
**Files**: `crates/director/src/error.rs`

**Current State**: Four `impl From<XError> for McpError` blocks (lines 8-39),
three of which are identical — just `McpError::internal_error(e.to_string(), None)`.
The `OperationError` match arm also maps every variant to `internal_error`.

**Target State**: A small macro:
```rust
macro_rules! impl_mcp_internal {
    ($($ty:ty),+) => { $(
        impl From<$ty> for McpError {
            fn from(e: $ty) -> Self {
                McpError::internal_error(e.to_string(), None)
            }
        }
    )+ };
}
impl_mcp_internal!(DaemonError, EditorError, OperationError);
```
Keep `ResolveError` separate since it maps to `invalid_params`.

**Approach**:
1. Add the macro
2. Replace the three identical impl blocks
3. Simplify `OperationError` — all variants map to the same thing, so
   the match is unnecessary

**Verification**:
- `cargo build -p director`
- `cargo test -p director`

---

## Dependency Order

Steps 1-3 are independent and can be done in any order or in parallel.
Step 4 is independent.
Step 5 depends on Step 3 (finalize_response signature may change).
Steps 6-8 are independent of each other and of Steps 1-5.

Recommended execution order: **1 → 2 → 3 → 4 → 5 → 6 → 7 → 8**

## Summary Table

| Step | Priority | Risk | Impact |
|------|----------|------|--------|
| 1. SQLite error helper | High | Low | -19 duplicate closures |
| 2. Watch remove contract fix | High | Low | Contract compliance |
| 3. Remove unwrap_or_default | High | Low | Error-layering compliance |
| 4. Shared serde helpers | Medium | Low | -2 cross-crate duplicates |
| 5. BudgetContext helper | Medium | Low | -16 lines across 8 handlers |
| 6. Unify query builders | Medium | Low | -15 lines, clearer structure |
| 7. Entity conversion module | Low | Low | Cleaner module boundaries |
| 8. Director error macro | Low | Low | -12 lines boilerplate |
