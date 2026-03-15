# Refactor Plan: Full Workspace

## Summary

Cross-crate analysis found six concrete refactoring opportunities: a contract
violation in distance field naming, duplicated serialization helpers between
stage-server and director, repeated static-cluster construction in the
clustering module, duplicated spatial-index rebuild logic between snapshot and
delta handlers, local default functions that belong in the shared defaults
module, and a useful `insert_if_nonempty` helper trapped in a single module.

All changes are internal — no MCP tool APIs, wire format, or GDScript addon
interfaces change (except fixing ClusterNearest's wire output from `dist` to
`distance`, which is a bug fix per contracts.md).

## Refactor Steps

### Step 1: Fix `dist` → `distance` contract violation

**Priority**: High
**Risk**: Low (internal rename + one wire format bug fix)
**Files**: `crates/stage-core/src/types.rs`, `crates/stage-core/src/cluster.rs`,
plus ~20 callsites across stage-core and stage-server

**Current State**:
- `RelativePosition::dist` uses `#[serde(rename = "distance")]` — the Rust
  field name violates the contract even though the wire format is correct.
- `ClusterNearest::dist` has **no serde rename** — the wire format outputs
  `"dist"`, violating contracts.md ("Distance Fields: Always `distance`").
- ~20 internal references use `.dist` throughout bearing, clustering, snapshot,
  inspect, delta, and clip_analysis modules.

**Target State**:
- Both structs use field name `distance` (no serde rename needed).
- All callsites updated from `.dist` to `.distance`.
- ClusterNearest wire output corrected from `"dist"` to `"distance"`.

**Approach**:
1. Rename `RelativePosition::dist` → `distance`, remove `#[serde(rename)]`.
2. Rename `ClusterNearest::dist` → `distance`.
3. Find-and-replace `.dist` → `.distance` across all Rust files that reference
   these structs. Also update test fixtures (`make_rel`, etc.).
4. Run `cargo test --workspace` to catch any missed references.

**Verification**:
- `cargo build --workspace` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
- `grep -r '\.dist[^a]' crates/` returns zero hits on RelativePosition/ClusterNearest usage

---

### Step 2: Extract static-geometry cluster construction

**Priority**: High
**Risk**: Low (pure refactor, no behavior change)
**Files**: `crates/stage-core/src/cluster.rs`

**Current State**:
The static_geometry cluster is constructed identically in 4 places
(cluster_by_group:77-86, cluster_by_class:160-169, cluster_by_proximity:234-243,
cluster_none:274-283):
```rust
clusters.push(Cluster {
    label: "static_geometry".to_string(),
    count: static_count,
    nearest: None,
    farthest_dist: 0.0,
    summary: None,
    note: Some("unchanged".to_string()),
});
```

**Target State**:
One `fn static_cluster(count: usize) -> Cluster` helper, called from all 4
clustering functions.

**Approach**:
1. Add `fn static_cluster(count: usize) -> Cluster` to cluster.rs.
2. Replace all 4 inline constructions with `clusters.push(static_cluster(static_count))`.
3. Existing tests cover all 4 clustering strategies — no new tests needed.

**Verification**:
- `cargo test -p stage-core` passes (existing cluster tests)
- `cargo clippy -p stage-core` clean

---

### Step 3: Consolidate default value functions

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/defaults.rs`,
`crates/stage-server/src/mcp/query.rs`

**Current State**:
- `defaults.rs` has: `default_perspective`, `default_radius`, `default_detail`
- `query.rs` has local: `default_k`, `default_query_radius` (lines 58-63)
- These local defaults should live alongside the others for discoverability.

**Target State**:
- `defaults.rs` contains all default functions: `default_perspective`,
  `default_radius`, `default_detail`, `default_k`, `default_query_radius`.
- `query.rs` imports from `super::defaults::*` instead of defining locally.

**Approach**:
1. Move `default_k` and `default_query_radius` from query.rs to defaults.rs.
2. Update query.rs imports.

**Verification**:
- `cargo build -p stage-server` passes
- `cargo test -p stage-server` passes

---

### Step 4: Extract `insert_if_nonempty` to shared MCP helpers

**Priority**: Medium
**Risk**: Low
**Files**: `crates/stage-server/src/mcp/delta.rs`,
`crates/stage-server/src/mcp/mod.rs`

**Current State**:
`insert_if_nonempty` (delta.rs:42-50) is a useful utility for conditionally
inserting non-empty arrays into JSON response objects. It's local to delta.rs
but the pattern appears informally elsewhere (e.g., snapshot response building).

**Target State**:
- `insert_if_nonempty` lives in `mcp/mod.rs` alongside other shared helpers.
- delta.rs uses `super::insert_if_nonempty`.

**Approach**:
1. Move `insert_if_nonempty` to `mcp/mod.rs` as `pub(crate)`.
2. Update delta.rs to use `super::insert_if_nonempty`.
3. Optionally adopt it in other handlers where the pattern appears inline.

**Verification**:
- `cargo build -p stage-server` passes
- `cargo test -p stage-server` passes

---

### Step 5: Deduplicate spatial-index rebuild between snapshot and delta

**Priority**: Medium
**Risk**: Medium (touches two critical handlers)
**Files**: `crates/stage-server/src/mcp/mod.rs` (snapshot handler, lines 217-259),
`crates/stage-server/src/mcp/delta.rs` (lines 141-153)

**Current State**:
Both snapshot and delta handlers rebuild the spatial index from raw entity data
using nearly identical code:
```rust
// snapshot (mod.rs:237-248)
let indexed: Vec<IndexedEntity> = raw_data.entities.iter()
    .map(|e| IndexedEntity { path, class, position: vec_to_array3(&e.position), groups })
    .collect();
state.spatial_index = SpatialIndex::build(indexed);

// delta (delta.rs:141-151) — identical
```

Snapshot also stores the delta baseline (`store_snapshot`) in the same block.
Delta does the same.

**Target State**:
A shared function (e.g., `update_spatial_state`) in a new module or in
`mcp/mod.rs` that:
1. Rebuilds the spatial index from `SnapshotResponse.entities`
2. Stores the delta baseline via `delta_engine.store_snapshot`
3. Returns the entity snapshots for further use

Both handlers call this shared function.

**Approach**:
1. Extract `fn update_spatial_state(state: &mut SessionState, raw_data: &SnapshotResponse) -> Vec<EntitySnapshot>` to `mcp/mod.rs` or a new `mcp/state.rs`.
2. Handle 2D vs 3D dimension check (snapshot does this, delta doesn't — delta
   always uses 3D, which may be a latent bug for 2D scenes).
3. Replace inline code in both handlers.

**Verification**:
- `cargo test --workspace` passes (especially E2E tests)
- Manual check: delta handler should respect `scene_dimensions` for 2D indexing

---

### Step 6: Extract shared MCP serialization helpers to a common module

**Priority**: Medium
**Risk**: Low (additive — new shared code, no behavior change)
**Files**: `crates/stage-server/src/mcp/mod.rs`,
`crates/director/src/mcp/mod.rs`

**Current State**:
`serialize_params` and `serialize_response` are duplicated 1:1 between
stage-server and director. Both have identical error handling.

**Target State**:
A shared module (options below) that both crates import from:
- **Option A**: New `crates/mcp-helpers/` micro-crate with `serialize_params`,
  `deserialize_response`, `serialize_response`.
- **Option B**: Move helpers into stage-protocol (both crates already
  depend on it), behind an optional `mcp` feature to avoid pulling McpError
  into protocol unconditionally.
- **Option C**: Accept the duplication — it's ~15 lines total and both crates
  are unlikely to diverge.

**Recommended**: Option C for now. The duplication is small and the alternatives
introduce either a new crate or a protocol→MCP dependency that doesn't belong.
Revisit if a third MCP server crate appears.

**Approach** (if choosing A or B):
1. Create shared module with the three helpers.
2. Update both crates to import from it.
3. Remove local definitions.

**Verification**:
- `cargo build --workspace` passes
- `cargo test --workspace` passes

---

## Non-Issues (Investigated, No Action Needed)

### Enum parser wrappers
Each handler module wraps `parse_enum_param` with type-specific variants.
This is intentional — it provides readable, type-safe interfaces per handler.
A macro would save ~3 lines per handler but reduce readability. Leave as-is.

### Handler pipeline (HandlerContext)
Multiple handlers follow the same config→query→finalize flow. A shared
`HandlerContext` builder would reduce boilerplate but add abstraction overhead.
The current explicit code is clear and each handler has legitimate variations.
Not worth abstracting until a third or fourth handler follows the exact same
sequence. Leave as-is.

### Activity logging
Already well-abstracted. Each tool has a dedicated summary function in
`activity.rs` and a standard `log_activity` call at the tail. No changes needed.

### TCP codec sync/async split
Intentional design — sync for GDExtension (no async runtime), async for server
(tokio). No duplication to resolve.

### Message construction helpers
Adding `Message::ok_response()` and `Message::error_response()` to
stage-protocol would be a minor improvement but the current usage is
minimal and clear. Low value.
