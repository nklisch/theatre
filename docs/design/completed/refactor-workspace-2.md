# Refactor Plan: Full Workspace

## Summary

Cross-crate analysis reveals three high-value refactoring targets: the director's
30+ tool handlers that are 100% copy-paste, enum parsing boilerplate across
stage-server, and distance rounding duplication. Medium-value improvements
include sharing error conversion macros, consolidating snapshot budget injection
paths, and extracting a `ProjectPathParams` trait for director.

## Refactor Steps

### Step 1: Director tool handler macro

**Priority**: High
**Risk**: Low
**Files**: `crates/director/src/mcp/mod.rs`

**Current State**: All 30 tool handlers in the `#[tool_router]` impl have an
identical 4-line body:

```rust
let op_params = serialize_params(&params)?;
let data = run_operation(&self.backend, &params.project_path, "<op>", &op_params).await?;
serialize_response(&data)
```

The only variance is the operation name string (which matches the function name)
and the params type. This is ~600 lines of pure boilerplate.

**Target State**: A `director_tool!` macro that generates the handler body:

```rust
macro_rules! director_tool {
    ($self:expr, $params:expr, $op:expr) => {{
        let op_params = serialize_params(&$params)?;
        let data = run_operation(&$self.backend, &$params.project_path, $op, &op_params).await?;
        serialize_response(&data)
    }};
}
```

Each handler becomes a one-liner:

```rust
pub async fn scene_create(&self, Parameters(params): Parameters<SceneCreateParams>) -> Result<String, McpError> {
    director_tool!(self, params, "scene_create")
}
```

**Approach**:
1. Add the macro at the top of `mod.rs` (private, not exported)
2. Replace all 30 handler bodies with the macro invocation
3. Keep `#[tool(description = ...)]` annotations unchanged

**Verification**:
- `cargo build -p director`
- `cargo test -p director`
- `cargo clippy -p director`
- Grep for `run_operation` — should only appear in the macro definition and `run_operation` fn itself

---

### Step 2: Extract `round_distance` utility

**Priority**: High (trivial, high clarity gain)
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/query.rs`, `crates/stage-core/src/bearing.rs`

**Current State**: Distance rounding `(x * 10.0).round() / 10.0` appears at:
- `query.rs:113` (query_result_entry)
- `query.rs:215` (relationship distance)
- `query.rs:235` (nav_distance)

**Target State**: A `round1` function in `stage_core::types`:

```rust
/// Round to 1 decimal place (0.1 precision).
pub fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
```

Called as `types::round1(distance)` at each site.

**Approach**:
1. Add `round1` to `stage-core/src/types.rs`
2. Replace the three inline expressions in `query.rs`

**Verification**:
- `cargo test -p stage-core`
- `cargo test -p stage-server`
- Grep for `* 10.0).round()` — should be zero occurrences outside the utility

---

### Step 3: Share `impl_mcp_internal!` macro via stage-protocol

**Priority**: Medium
**Risk**: Low
**Files**: `crates/director/src/error.rs`, `crates/stage-protocol/src/mcp_helpers.rs`

**Current State**: Director defines `impl_mcp_internal!` locally in `error.rs` to
convert error types to `McpError::internal_error`. Stage-server uses ad-hoc
`.map_err(|e| McpError::internal_error(...))` inline.

**Target State**: `impl_mcp_internal!` lives in `stage-protocol::mcp_helpers`
(behind the `mcp` feature flag, since it depends on `rmcp`). Director re-exports
from there. Stage-server can use it for any future error conversions.

**Approach**:
1. Move the macro to `stage-protocol/src/mcp_helpers.rs`
2. Export it: `pub use mcp_helpers::impl_mcp_internal;`
3. Update `director/src/error.rs` to import from `stage_protocol`
4. Keep director's `From<ResolveError>` hand-written (it maps to `invalid_params`, not `internal_error`)

**Verification**:
- `cargo build --workspace`
- `cargo test --workspace`

---

### Step 4: Consolidate enum parsing with `FromStr`-style trait

**Priority**: Medium
**Risk**: Medium (touches many modules)
**Files**: `crates/stage-server/src/mcp/mod.rs`, `*/config.rs`, `*/watch.rs`,
`*/scene_tree.rs`, `*/snapshot.rs`, `*/inspect.rs`

**Current State**: `parse_enum_param` + `parse_enum_list` at `mod.rs:72-102` are
generic enough, but each call site wraps them in a thin function like
`parse_detail`, `parse_cluster_by`, `parse_operator`, `parse_track`,
`parse_action`, `parse_include_type`. Each wrapper just passes a different
variants array.

**Target State**: Add a `ParseMcpEnum` trait that enum types can implement:

```rust
pub trait ParseMcpEnum: Sized {
    const FIELD_NAME: &'static str;
    fn variants() -> &'static [(&'static str, Self)];

    fn parse(s: &str) -> Result<Self, McpError> {
        parse_enum_param(s, Self::FIELD_NAME, Self::variants())
    }

    fn parse_list(values: &[String]) -> Result<Vec<Self>, McpError> {
        parse_enum_list(values, Self::FIELD_NAME, Self::variants())
    }
}
```

Each enum implements it (can be a manual impl or a simple macro). The call sites
become `DetailLevel::parse(&params.detail)?` instead of `parse_detail(&params.detail)?`.

**Approach**:
1. Define `ParseMcpEnum` trait in `mcp/mod.rs`
2. Implement for: `DetailLevel`, `ClusterBy`, `BearingFormat`, `WatchOperator`,
   `TrackKind`, `SceneTreeAction`, `InspectInclude`
3. Replace wrapper functions with trait method calls
4. Remove the now-unused wrapper functions

**Verification**:
- `cargo build -p stage-server`
- `cargo test -p stage-server`
- `cargo clippy -p stage-server`
- Grep for `parse_enum_param` — should only appear in the trait default impl

---

### Step 5: Unify snapshot budget injection path

**Priority**: Medium
**Risk**: Medium
**Files**: `crates/stage-server/src/mcp/mod.rs:300-347`,
`crates/stage-server/src/mcp/snapshot.rs`

**Current State**: The `spatial_snapshot` handler has two code paths that call
`serialize_response` directly (lines 310, 344) instead of `finalize_response`.
This works because the snapshot builder functions (`build_summary_response`,
`build_snapshot_body`, `build_expand_response`) inject their own
`"budget": enforcer.report()` block. But this creates two different budget
injection mechanisms — `finalize_response` (used by all other tools) and
`BudgetEnforcer::report()` (used by snapshot builders).

**Target State**: Snapshot builders return their response JSON without the budget
block. The handler calls `finalize_response` uniformly like all other tools.
`BudgetEnforcer::report()` returns used/limit data that can be consumed by
`inject_budget`.

**Approach**:
1. Modify snapshot builder functions to omit the `"budget"` key from their output
2. Have builders return `(serde_json::Value, u32)` — the response and the token
   count from the enforcer
3. In `spatial_snapshot`, call `inject_budget` with the enforcer's used count,
   then `serialize_response`
4. Alternatively, keep `finalize_response` as the single budget injection point

**Verification**:
- `cargo test -p stage-server` — all snapshot tests pass
- E2E journey tests pass
- Manual verification: snapshot responses still contain `budget` block with
  correct `used`, `limit`, `hard_cap`

---

### Step 6: Rename internal protocol `id` to `request_id`

**Priority**: Low
**Risk**: High (wire format change, requires coordinated addon + server update)
**Files**: `crates/stage-protocol/src/messages.rs`,
`crates/stage-server/src/tcp.rs`, `crates/stage-godot/src/tcp_server.rs`,
`crates/stage-godot/src/query_handler.rs`

**Current State**: `Message::Query`, `Message::Response`, and `Message::Error`
use bare `id: String` for request correlation. This violates the contracts rule
requiring `<resource>_id` naming.

**Target State**: All three use `request_id: String` with
`#[serde(alias = "id")]` for backwards compatibility during rollout.

**Approach**:
1. Rename `id` → `request_id` in the `Message` enum variants
2. Add `#[serde(alias = "id")]` for deserialization compatibility
3. Update all construction sites in `tcp.rs` and `tcp_server.rs`
4. Update all pattern-match destructures
5. After one release cycle, remove the `alias`

**Verification**:
- `cargo test --workspace`
- E2E journey tests (live addon ↔ server communication)
- Wire format: verify JSON output uses `"request_id"` in new messages

---

## Priority Summary

| Step | Priority | Risk | LOC Impact | Dependencies |
|------|----------|------|-----------|--------------|
| 1. Director tool macro | High | Low | −500 | None |
| 2. round_distance utility | High | Low | −5 | None |
| 3. Share impl_mcp_internal | Medium | Low | −10 | None |
| 4. ParseMcpEnum trait | Medium | Medium | −60 | None |
| 5. Snapshot budget path | Medium | Medium | ~0 (restructure) | None |
| 6. Protocol request_id | Low | High | ~+20 | Coordinated deploy |

Steps 1–3 are independent and can be done in any order.
Step 4 is independent.
Step 5 is independent.
Step 6 should be done last and requires a coordinated addon + server deploy.
