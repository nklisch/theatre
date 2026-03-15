# Design: Milestone 4 — Deltas & Watches

## Overview

M4 delivers three things: the **delta engine** (tracking state changes between queries), the **watch system** (conditional subscriptions), and **`return_delta` wiring** for `spatial_action`. These make iterative debugging possible — act, check, act, check.

Two new MCP tools: `spatial_delta` and `spatial_watch`.
One enhancement: `return_delta` on `spatial_action` responses.

**Depends on:** M1 (TCP flow, snapshot data, spatial index, budget), M3 (action responses, `return_delta` placeholder)

**Exit Criteria:** Agent sets up watches on enemy group, advances game time, calls `spatial_delta()` — sees movement, state changes, and watch triggers. Agent's watch fires when enemy health drops below 20. `return_delta` on teleport shows immediate spatial consequences.

---

## Implementation Units

### Unit 1: Snapshot State Storage (`stage-core`)

**File:** `crates/stage-core/src/delta.rs` (new)

The delta engine needs to store the "last known state" per entity to compute diffs. This module tracks entity snapshots and computes changes.

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::Position3;

// -----------------------------------------------------------------------
// Change detection thresholds (from SPEC.md)
// -----------------------------------------------------------------------

/// Position change below this is suppressed (world units).
pub const POSITION_THRESHOLD: f64 = 0.01;
/// Rotation change below this is suppressed (degrees).
pub const ROTATION_THRESHOLD: f64 = 0.1;
/// Float property change below this is suppressed.
pub const FLOAT_THRESHOLD: f64 = 0.001;

// -----------------------------------------------------------------------
// Stored entity state
// -----------------------------------------------------------------------

/// Minimal snapshot of one entity's state, stored between queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub rotation_deg: [f64; 3],
    pub velocity: [f64; 3],
    pub groups: Vec<String>,
    /// Exported variable state.
    pub state: serde_json::Map<String, serde_json::Value>,
    pub visible: bool,
}

/// Event buffered between delta queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferedEvent {
    pub event_type: BufferedEventType,
    pub path: String,
    pub frame: u64,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferedEventType {
    SignalEmitted,
    NodeEntered,
    NodeExited,
}

// -----------------------------------------------------------------------
// Delta engine
// -----------------------------------------------------------------------

/// The delta engine: stores last-query state, computes diffs.
pub struct DeltaEngine {
    /// Entity state at the time of the last snapshot/delta.
    last_snapshot: HashMap<String, EntitySnapshot>,
    /// Frame number of the last stored snapshot.
    last_frame: u64,
    /// Buffered events received between queries (from push events).
    event_buffer: Vec<BufferedEvent>,
}

impl DeltaEngine {
    pub fn new() -> Self {
        Self {
            last_snapshot: HashMap::new(),
            last_frame: 0,
            event_buffer: Vec::new(),
        }
    }

    /// Store a new snapshot. Called after every `spatial_snapshot` or
    /// `spatial_delta` query completes.
    pub fn store_snapshot(&mut self, frame: u64, entities: Vec<EntitySnapshot>) {
        self.last_snapshot.clear();
        for entity in entities {
            self.last_snapshot.insert(entity.path.clone(), entity);
        }
        self.last_frame = frame;
        self.event_buffer.clear();
    }

    /// Push an event into the buffer (from addon push events).
    pub fn push_event(&mut self, event: BufferedEvent) {
        self.event_buffer.push(event);
    }

    /// Returns the frame of the last stored snapshot.
    pub fn last_frame(&self) -> u64 {
        self.last_frame
    }

    /// Returns true if we have a stored snapshot to diff against.
    pub fn has_baseline(&self) -> bool {
        self.last_frame > 0
    }

    /// Drain the buffered events (consumed by delta computation).
    pub fn drain_events(&mut self) -> Vec<BufferedEvent> {
        std::mem::take(&mut self.event_buffer)
    }

    /// Compute changes between stored snapshot and a new set of entities.
    /// Returns categorized changes.
    pub fn compute_delta(
        &self,
        current_entities: &[EntitySnapshot],
        current_frame: u64,
    ) -> DeltaResult {
        let mut moved = Vec::new();
        let mut state_changed = Vec::new();
        let mut entered = Vec::new();
        let mut exited = Vec::new();

        // Build a set of current paths for exit detection
        let current_paths: std::collections::HashSet<&str> =
            current_entities.iter().map(|e| e.path.as_str()).collect();

        // Check each current entity against stored state
        for entity in current_entities {
            match self.last_snapshot.get(&entity.path) {
                Some(prev) => {
                    // Check movement
                    if let Some(movement) = detect_movement(prev, entity) {
                        moved.push(movement);
                    }
                    // Check state changes
                    if let Some(changes) = detect_state_changes(prev, entity) {
                        state_changed.push(changes);
                    }
                }
                None => {
                    // New entity — entered
                    entered.push(EnteredEntity {
                        path: entity.path.clone(),
                        class: entity.class.clone(),
                        position: entity.position,
                    });
                }
            }
        }

        // Check for exited entities (were in last snapshot, not in current)
        for (path, prev) in &self.last_snapshot {
            if !current_paths.contains(path.as_str()) {
                exited.push(ExitedEntity {
                    path: path.clone(),
                    reason: "removed".to_string(),
                });
            }
        }

        DeltaResult {
            from_frame: self.last_frame,
            to_frame: current_frame,
            moved,
            state_changed,
            entered,
            exited,
        }
    }
}

impl Default for DeltaEngine {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------
// Delta result types
// -----------------------------------------------------------------------

/// Result of a delta computation.
#[derive(Debug, Clone, Serialize)]
pub struct DeltaResult {
    pub from_frame: u64,
    pub to_frame: u64,
    pub moved: Vec<MovedEntity>,
    pub state_changed: Vec<StateChange>,
    pub entered: Vec<EnteredEntity>,
    pub exited: Vec<ExitedEntity>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MovedEntity {
    pub path: String,
    /// New position.
    pub pos: Position3,
    /// Position delta (new - old).
    pub delta_pos: Position3,
    /// Distance from focal point (filled in by server, not by engine).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_to_focal: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateChange {
    pub path: String,
    /// Map of property_name → [old_value, new_value].
    pub changes: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnteredEntity {
    pub path: String,
    pub class: String,
    pub position: Position3,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExitedEntity {
    pub path: String,
    pub reason: String,
}

// -----------------------------------------------------------------------
// Change detection helpers
// -----------------------------------------------------------------------

/// Detect movement beyond threshold.
fn detect_movement(prev: &EntitySnapshot, curr: &EntitySnapshot) -> Option<MovedEntity> {
    let dx = curr.position[0] - prev.position[0];
    let dy = curr.position[1] - prev.position[1];
    let dz = curr.position[2] - prev.position[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

    if dist < POSITION_THRESHOLD {
        return None;
    }

    Some(MovedEntity {
        path: curr.path.clone(),
        pos: curr.position,
        delta_pos: [dx, dy, dz],
        dist_to_focal: None, // Filled in by the server layer
    })
}

/// Detect state property changes beyond thresholds.
fn detect_state_changes(prev: &EntitySnapshot, curr: &EntitySnapshot) -> Option<StateChange> {
    let mut changes = serde_json::Map::new();

    // Check properties in current state
    for (key, new_val) in &curr.state {
        match prev.state.get(key) {
            Some(old_val) => {
                if !values_equal(old_val, new_val) {
                    changes.insert(
                        key.clone(),
                        serde_json::json!([old_val, new_val]),
                    );
                }
            }
            None => {
                // New property appeared
                changes.insert(
                    key.clone(),
                    serde_json::json!([null, new_val]),
                );
            }
        }
    }

    // Check for removed properties
    for (key, old_val) in &prev.state {
        if !curr.state.contains_key(key) {
            changes.insert(
                key.clone(),
                serde_json::json!([old_val, null]),
            );
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(StateChange {
            path: curr.path.clone(),
            changes,
        })
    }
}

/// Compare two JSON values with float thresholds.
fn values_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    match (a, b) {
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            match (a.as_f64(), b.as_f64()) {
                (Some(fa), Some(fb)) => (fa - fb).abs() < FLOAT_THRESHOLD,
                _ => a == b,
            }
        }
        _ => a == b,
    }
}
```

**Implementation Notes:**
- `DeltaEngine` lives in stage-core (pure Rust, no Godot/MCP deps)
- `store_snapshot` is called after every `spatial_snapshot` and `spatial_delta` so the baseline is always fresh
- `compute_delta` is pure: it takes current data and compares against stored state
- The `event_buffer` is filled by push events from the addon (signal subscriptions, future M4 addition)
- Thresholds match SPEC.md: position < 0.01, rotation < 0.1°, float < 0.001
- `dist_to_focal` is `Option` — set by the server MCP layer, not the delta engine

**Acceptance Criteria:**
- [ ] `DeltaEngine::store_snapshot` stores entity state keyed by path
- [ ] `DeltaEngine::compute_delta` detects moved entities beyond threshold
- [ ] `compute_delta` detects state property changes with float thresholds
- [ ] `compute_delta` detects entered (new) and exited (removed) entities
- [ ] Position changes below 0.01 units are suppressed
- [ ] Float property changes below 0.001 are suppressed
- [ ] `drain_events` returns and clears the event buffer

---

### Unit 2: Watch Engine (`stage-core`)

**File:** `crates/stage-core/src/watch.rs` (new)

The watch engine manages subscriptions and evaluates conditions against entity state. It is pure computation — no Godot or MCP deps.

```rust
use serde::{Deserialize, Serialize};

use crate::delta::EntitySnapshot;

/// Unique watch identifier.
pub type WatchId = String;

/// A watch subscription.
#[derive(Debug, Clone, Serialize)]
pub struct Watch {
    pub id: WatchId,
    /// Node path or "group:<name>".
    pub node: String,
    /// Conditions that must be met for a trigger.
    pub conditions: Vec<WatchCondition>,
    /// What aspects to track.
    pub track: Vec<TrackCategory>,
}

/// A condition on a watch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchCondition {
    pub property: String,
    pub operator: ConditionOperator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Lt,
    Gt,
    Eq,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackCategory {
    Position,
    State,
    Signals,
    Physics,
    All,
}

/// A triggered watch result, included in delta responses.
#[derive(Debug, Clone, Serialize)]
pub struct WatchTrigger {
    pub watch_id: WatchId,
    pub node: String,
    pub trigger: String,
    pub frame: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_state: Option<serde_json::Value>,
}

/// The watch engine: manages subscriptions and evaluates conditions.
pub struct WatchEngine {
    watches: Vec<Watch>,
    next_id: u64,
}

impl WatchEngine {
    pub fn new() -> Self {
        Self {
            watches: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new watch. Returns the watch ID.
    pub fn add(&mut self, node: String, conditions: Vec<WatchCondition>, track: Vec<TrackCategory>) -> Watch {
        let id = format!("w_{:03}", self.next_id);
        self.next_id += 1;

        let watch = Watch {
            id,
            node,
            conditions,
            track,
        };
        self.watches.push(watch.clone());
        watch
    }

    /// Remove a watch by ID. Returns true if found and removed.
    pub fn remove(&mut self, watch_id: &str) -> bool {
        let len_before = self.watches.len();
        self.watches.retain(|w| w.id != watch_id);
        self.watches.len() < len_before
    }

    /// List all active watches.
    pub fn list(&self) -> &[Watch] {
        &self.watches
    }

    /// Remove all watches. Returns the count removed.
    pub fn clear(&mut self) -> usize {
        let count = self.watches.len();
        self.watches.clear();
        count
    }

    /// Evaluate all watches against current entity state.
    /// `prev_state` is the previous entity map (for "changed" operator).
    /// `curr_state` is the current entity map.
    /// Returns all triggered watches.
    pub fn evaluate(
        &self,
        prev_state: &std::collections::HashMap<String, EntitySnapshot>,
        curr_state: &[EntitySnapshot],
        frame: u64,
    ) -> Vec<WatchTrigger> {
        let mut triggers = Vec::new();

        for watch in &self.watches {
            let matching_entities = self.resolve_watch_targets(&watch.node, curr_state);

            for entity in &matching_entities {
                for condition in &watch.conditions {
                    if let Some(trigger_msg) = evaluate_condition(
                        condition,
                        entity,
                        prev_state.get(&entity.path),
                    ) {
                        triggers.push(WatchTrigger {
                            watch_id: watch.id.clone(),
                            node: entity.path.clone(),
                            trigger: trigger_msg,
                            frame,
                            full_state: Some(serde_json::to_value(entity).unwrap_or_default()),
                        });
                    }
                }

                // If no conditions, any tracked change triggers
                if watch.conditions.is_empty() {
                    // No-condition watches are always "active" — they just
                    // ensure the entity's changes appear in delta responses.
                    // No trigger is generated (the entity is simply included).
                }
            }
        }

        triggers
    }

    /// Get all node paths that are being watched (for ensuring they appear in deltas).
    /// Expands group watches into the matching entity paths.
    pub fn watched_paths(&self, all_entities: &[EntitySnapshot]) -> Vec<String> {
        let mut paths = Vec::new();
        for watch in &self.watches {
            let targets = self.resolve_watch_targets(&watch.node, all_entities);
            for entity in targets {
                if !paths.contains(&entity.path) {
                    paths.push(entity.path.clone());
                }
            }
        }
        paths
    }

    /// Returns serializable watch list for reconnection re-send.
    pub fn watches_for_reconnect(&self) -> &[Watch] {
        &self.watches
    }

    /// Resolve a watch target ("group:enemies" or "enemies/scout_02") to
    /// matching entities from the current state.
    fn resolve_watch_targets<'a>(
        &self,
        target: &str,
        entities: &'a [EntitySnapshot],
    ) -> Vec<&'a EntitySnapshot> {
        if let Some(group) = target.strip_prefix("group:") {
            entities
                .iter()
                .filter(|e| e.groups.iter().any(|g| g == group))
                .collect()
        } else {
            entities
                .iter()
                .filter(|e| e.path == target)
                .collect()
        }
    }
}

impl Default for WatchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate a single condition against an entity.
/// Returns Some(trigger_message) if condition is met, None otherwise.
fn evaluate_condition(
    condition: &WatchCondition,
    entity: &EntitySnapshot,
    prev: Option<&EntitySnapshot>,
) -> Option<String> {
    let current_value = entity.state.get(&condition.property)?;

    match condition.operator {
        ConditionOperator::Changed => {
            let prev_value = prev.and_then(|p| p.state.get(&condition.property));
            match prev_value {
                Some(old) if !super::delta::values_equal(old, current_value) => {
                    Some(format!(
                        "{} changed from {} to {}",
                        condition.property, old, current_value
                    ))
                }
                None => Some(format!("{} appeared with value {}", condition.property, current_value)),
                _ => None,
            }
        }
        ConditionOperator::Lt => {
            let threshold = condition.value.as_ref()?.as_f64()?;
            let current = current_value.as_f64()?;
            if current < threshold {
                Some(format!(
                    "{} dropped to {} (threshold: < {})",
                    condition.property, current, threshold
                ))
            } else {
                None
            }
        }
        ConditionOperator::Gt => {
            let threshold = condition.value.as_ref()?.as_f64()?;
            let current = current_value.as_f64()?;
            if current > threshold {
                Some(format!(
                    "{} rose to {} (threshold: > {})",
                    condition.property, current, threshold
                ))
            } else {
                None
            }
        }
        ConditionOperator::Eq => {
            let target = condition.value.as_ref()?;
            if current_value == target {
                Some(format!("{} equals {}", condition.property, current_value))
            } else {
                None
            }
        }
    }
}
```

**Implementation Notes:**
- `values_equal` is imported from `delta` module (make it `pub`)
- Group watches use `"group:<name>"` prefix convention from CONTRACT.md
- `resolve_watch_targets` handles both individual node paths and group watches
- Condition-free watches don't trigger — they just ensure entities appear in delta results
- The watch engine is stateless regarding entity data; it only stores subscriptions
- `watched_paths` is used by the delta MCP handler to ensure watched entities are always fetched, even if they'd otherwise be filtered out

**Acceptance Criteria:**
- [ ] `add` creates a watch with auto-incrementing ID (`w_001`, `w_002`, ...)
- [ ] `remove` removes a specific watch by ID, returns true if found
- [ ] `clear` removes all watches, returns count
- [ ] `list` returns all active watches
- [ ] `evaluate` with `Lt` condition triggers when property < threshold
- [ ] `evaluate` with `Gt` condition triggers when property > threshold
- [ ] `evaluate` with `Eq` condition triggers when property equals value
- [ ] `evaluate` with `Changed` condition triggers on value change
- [ ] Group watches (`"group:enemies"`) match all entities in that group
- [ ] Node watches (`"enemies/scout_02"`) match exact path

---

### Unit 3: Wire `delta` and `watch` into `stage-core` exports

**File:** `crates/stage-core/src/lib.rs` (edit)

```rust
pub mod bearing;
pub mod budget;
pub mod cluster;
pub mod delta;
pub mod index;
pub mod types;
pub mod watch;
```

**File:** `crates/stage-core/src/delta.rs` — make `values_equal` public:

```rust
pub fn values_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
```

**Acceptance Criteria:**
- [ ] `stage_core::delta` and `stage_core::watch` are accessible from `stage-server`
- [ ] Workspace builds with `cargo build --workspace`

---

### Unit 4: Add Delta Engine and Watch Engine to SessionState (`stage-server`)

**File:** `crates/stage-server/src/tcp.rs` (edit)

Add `DeltaEngine` and `WatchEngine` to `SessionState`:

```rust
use stage_core::delta::DeltaEngine;
use stage_core::watch::WatchEngine;

pub struct SessionState {
    pub tcp_writer: Option<TcpClientHandle>,
    pub connected: bool,
    pub session_id: Option<String>,
    pub handshake_info: Option<HandshakeInfo>,
    pub pending_queries: HashMap<String, oneshot::Sender<QueryResult>>,
    pub spatial_index: SpatialIndex,
    /// Delta engine: tracks entity state changes between queries.
    pub delta_engine: DeltaEngine,
    /// Watch engine: manages watch subscriptions and evaluates conditions.
    pub watch_engine: WatchEngine,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            tcp_writer: None,
            connected: false,
            session_id: None,
            handshake_info: None,
            pending_queries: HashMap::new(),
            spatial_index: SpatialIndex::empty(),
            delta_engine: DeltaEngine::new(),
            watch_engine: WatchEngine::new(),
        }
    }
}
```

**Implementation Notes:**
- The delta engine and watch engine persist across reconnections (per design: "Session state (watches, config) preserved in memory")
- On reconnection, watches are NOT cleared — they survive game restart
- The delta engine's baseline IS cleared on reconnect (since game state resets)

Also add reconnection handling in the `tcp_client_loop` disconnect cleanup:

```rust
// In handle_connection cleanup (after disconnect):
{
    let mut s = state.lock().await;
    s.tcp_writer = None;
    s.connected = false;
    s.pending_queries.clear();
    // Clear delta baseline on disconnect (game state resets)
    s.delta_engine = DeltaEngine::new();
    // Watches persist — will be re-evaluated after reconnect
}
```

**Acceptance Criteria:**
- [ ] `SessionState` has `delta_engine` and `watch_engine` fields
- [ ] Delta engine baseline is cleared on TCP disconnect
- [ ] Watch engine persists across reconnections

---

### Unit 5: Store Snapshot in Delta Engine (`stage-server`)

**File:** `crates/stage-server/src/mcp/snapshot.rs` (edit)

After a `spatial_snapshot` completes and the spatial index is rebuilt, store the entity data in the delta engine. This is the auto-update mechanism — every snapshot call automatically becomes the new delta baseline.

Add a helper to convert `EntityData` to `EntitySnapshot`:

```rust
use stage_core::delta::EntitySnapshot;
use stage_core::types::vec_to_array3;

/// Convert protocol EntityData to a delta-compatible EntitySnapshot.
pub fn to_entity_snapshot(e: &EntityData) -> EntitySnapshot {
    EntitySnapshot {
        path: e.path.clone(),
        class: e.class.clone(),
        position: vec_to_array3(&e.position),
        rotation_deg: vec_to_array3(&e.rotation_deg),
        velocity: vec_to_array3(&e.velocity),
        groups: e.groups.clone(),
        state: e.state.clone(),
        visible: e.visible,
    }
}
```

In the `spatial_snapshot` handler, after rebuilding the spatial index (step 6b), add:

```rust
// 6c. Store snapshot in delta engine for subsequent delta queries
{
    let snapshots: Vec<EntitySnapshot> = raw_data
        .entities
        .iter()
        .map(to_entity_snapshot)
        .collect();
    let mut state = self.state.lock().await;
    state.delta_engine.store_snapshot(raw_data.frame, snapshots);
}
```

**Implementation Notes:**
- This uses the same lock acquisition as the spatial index rebuild (step 6b). These two operations can share a single lock by combining them, but for clarity they can be separate (the lock is brief).
- Every snapshot call updates the delta baseline, so the agent's workflow of `snapshot → act → delta` works naturally.

**Acceptance Criteria:**
- [ ] After `spatial_snapshot`, delta engine has stored entity state
- [ ] Subsequent `spatial_delta` has a baseline to diff against
- [ ] Frame number is recorded for `since_frame` support

---

### Unit 6: `spatial_delta` MCP Tool (`stage-server`)

**File:** `crates/stage-server/src/mcp/delta.rs` (new)

```rust
use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use stage_core::{
    bearing,
    budget::{estimate_tokens, resolve_budget, SnapshotBudgetDefaults},
    delta::{DeltaResult, EntitySnapshot},
    types::vec_to_array3,
};
use stage_protocol::query::{GetSnapshotDataParams, DetailLevel, PerspectiveParam};

use crate::tcp::query_addon;
use crate::server::StageServer;

use super::{deserialize_response, inject_budget, serialize_params, serialize_response};
use super::snapshot::to_entity_snapshot;

/// MCP parameters for the spatial_delta tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialDeltaParams {
    /// Frame to diff against. If omitted, diffs against the last query.
    pub since_frame: Option<u64>,

    /// Perspective type: "camera", "node", or "point". Default: "camera".
    #[serde(default = "default_perspective")]
    pub perspective: String,

    /// Max distance from perspective. Default: 50.0.
    #[serde(default = "default_radius")]
    pub radius: f64,

    /// Filter by group membership.
    pub groups: Option<Vec<String>>,

    /// Filter by node class.
    pub class_filter: Option<Vec<String>>,

    /// Soft token budget override.
    pub token_budget: Option<u32>,
}

fn default_perspective() -> String { "camera".to_string() }
fn default_radius() -> f64 { 50.0 }
```

In `crates/stage-server/src/mcp/mod.rs`, add the delta module and tool registration:

```rust
pub mod delta;
// ...existing modules...

use delta::SpatialDeltaParams;
```

Add the tool method to the `#[tool_router]` impl block:

```rust
/// See what changed since the last query. Returns moved entities, state
/// changes, new/removed nodes, emitted signals, and watch triggers.
#[tool(description = "See what changed since the last query. Returns moved entities, state changes, new/removed nodes, and watch triggers. Use after spatial_snapshot or spatial_action to see effects. Use since_frame to diff against a specific frame.")]
pub async fn spatial_delta(
    &self,
    Parameters(params): Parameters<SpatialDeltaParams>,
) -> Result<String, McpError> {
    delta::handle_spatial_delta(params, &self.state).await
}
```

The handler implementation in `delta.rs`:

```rust
pub async fn handle_spatial_delta(
    params: SpatialDeltaParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    // 1. Check we have a baseline
    {
        let s = state.lock().await;
        if !s.delta_engine.has_baseline() {
            return Err(McpError::invalid_params(
                "No baseline snapshot available. Call spatial_snapshot first, \
                 then spatial_delta to see what changed.",
                None,
            ));
        }
    }

    // 2. Build perspective param for addon query
    let perspective_param = match params.perspective.as_str() {
        "camera" => PerspectiveParam::Camera,
        "node" => {
            return Err(McpError::invalid_params(
                "Node perspective on delta requires focal_node (not yet supported — use 'camera' or 'point')",
                None,
            ));
        }
        _ => PerspectiveParam::Camera,
    };

    // 3. Query addon for current state
    let query_params = GetSnapshotDataParams {
        perspective: perspective_param,
        radius: params.radius,
        include_offscreen: true, // Delta needs all entities, not just visible
        groups: params.groups.clone().unwrap_or_default(),
        class_filter: params.class_filter.clone().unwrap_or_default(),
        detail: DetailLevel::Standard,
    };

    let raw_data: stage_protocol::query::SnapshotResponse = {
        let data = query_addon(state, "get_snapshot_data", serialize_params(&query_params)?)
            .await?;
        deserialize_response(data)?
    };

    // 4. Convert to entity snapshots
    let current_snapshots: Vec<EntitySnapshot> = raw_data
        .entities
        .iter()
        .map(to_entity_snapshot)
        .collect();

    // 5. Compute delta and evaluate watches
    let (delta_result, watch_triggers, from_frame) = {
        let mut s = state.lock().await;

        // Compute delta
        let delta = s.delta_engine.compute_delta(&current_snapshots, raw_data.frame);

        // Evaluate watches
        let triggers = s.watch_engine.evaluate(
            &s.delta_engine.last_snapshot_map(),
            &current_snapshots,
            raw_data.frame,
        );

        let from = delta.from_frame;

        // Update delta engine with new baseline
        s.delta_engine.store_snapshot(raw_data.frame, current_snapshots.clone());

        // Rebuild spatial index
        let indexed: Vec<stage_core::index::IndexedEntity> = raw_data
            .entities
            .iter()
            .map(|e| stage_core::index::IndexedEntity {
                path: e.path.clone(),
                class: e.class.clone(),
                position: vec_to_array3(&e.position),
                groups: e.groups.clone(),
            })
            .collect();
        s.spatial_index = stage_core::index::SpatialIndex::build(indexed);

        (delta, triggers, from)
    };

    // 6. Build response
    let budget_limit = resolve_budget(params.token_budget, 1000, SnapshotBudgetDefaults::HARD_CAP);

    let dt_ms = raw_data.timestamp_ms.saturating_sub(0); // Need from_timestamp for proper dt

    let mut response = serde_json::json!({
        "from_frame": delta_result.from_frame,
        "to_frame": delta_result.to_frame,
    });

    if let serde_json::Value::Object(ref mut map) = response {
        if !delta_result.moved.is_empty() {
            map.insert("moved".into(), serde_json::to_value(&delta_result.moved).unwrap_or_default());
        }
        if !delta_result.state_changed.is_empty() {
            map.insert("state_changed".into(), serde_json::to_value(&delta_result.state_changed).unwrap_or_default());
        }
        if !delta_result.entered.is_empty() {
            map.insert("entered".into(), serde_json::to_value(&delta_result.entered).unwrap_or_default());
        }
        if !delta_result.exited.is_empty() {
            map.insert("exited".into(), serde_json::to_value(&delta_result.exited).unwrap_or_default());
        }

        map.insert("static_changed".into(), serde_json::json!(false));

        if !watch_triggers.is_empty() {
            map.insert("watch_triggers".into(), serde_json::to_value(&watch_triggers).unwrap_or_default());
        }
    }

    // 7. Budget
    let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
    let used = estimate_tokens(json_bytes);
    inject_budget(&mut response, used, budget_limit);

    serialize_response(&response)
}
```

**Implementation Notes:**
- The handler makes `DeltaEngine.last_snapshot` accessible as a HashMap — add a `pub fn last_snapshot_map(&self) -> &HashMap<String, EntitySnapshot>` getter to the delta engine
- Delta always fetches with `include_offscreen: true` because we need to detect all entities for exit detection, not just visible ones
- After computing the delta, the new state becomes the baseline (`store_snapshot`)
- The spatial index is also rebuilt during delta (same pattern as snapshot)
- `since_frame` is checked: if specified and different from `last_frame`, we could return an error or still compute against the stored baseline (simplest: ignore `since_frame` for now, always diff against last)
- Empty categories are omitted from the response for token efficiency

**Acceptance Criteria:**
- [ ] `spatial_delta()` returns moved, state_changed, entered, exited entities
- [ ] Returns error if no baseline snapshot exists (agent must call `spatial_snapshot` first)
- [ ] After delta, the new state becomes the baseline for the next delta
- [ ] Watch triggers appear in `watch_triggers` array
- [ ] Empty change categories are omitted from response
- [ ] `budget` block is present on response
- [ ] Spatial index is rebuilt during delta query

---

### Unit 7: `spatial_watch` MCP Tool (`stage-server`)

**File:** `crates/stage-server/src/mcp/watch.rs` (new)

```rust
use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use stage_core::{
    budget::estimate_tokens,
    watch::{ConditionOperator, TrackCategory, WatchCondition},
};

use crate::server::StageServer;

use super::{inject_budget, serialize_response};

/// MCP parameters for the spatial_watch tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialWatchParams {
    /// Action: "add", "remove", "list", "clear".
    #[schemars(description = "Action: add, remove, list, clear")]
    pub action: String,

    /// For "add": watch specification.
    pub watch: Option<WatchSpec>,

    /// For "remove": watch ID to remove.
    pub watch_id: Option<String>,
}

/// Watch specification for the "add" action.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchSpec {
    /// Node path or "group:<name>".
    pub node: String,

    /// Conditions for triggering.
    #[serde(default)]
    pub conditions: Vec<WatchConditionInput>,

    /// What to track: position, state, signals, physics, all.
    #[serde(default = "default_track")]
    pub track: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchConditionInput {
    pub property: String,
    /// Operator: "lt", "gt", "eq", "changed".
    pub operator: String,
    pub value: Option<serde_json::Value>,
}

fn default_track() -> Vec<String> {
    vec!["all".to_string()]
}

fn parse_operator(s: &str) -> Result<ConditionOperator, McpError> {
    match s {
        "lt" => Ok(ConditionOperator::Lt),
        "gt" => Ok(ConditionOperator::Gt),
        "eq" => Ok(ConditionOperator::Eq),
        "changed" => Ok(ConditionOperator::Changed),
        other => Err(McpError::invalid_params(
            format!("Unknown operator '{other}'. Valid: lt, gt, eq, changed"),
            None,
        )),
    }
}

fn parse_track(s: &str) -> Result<TrackCategory, McpError> {
    match s {
        "position" => Ok(TrackCategory::Position),
        "state" => Ok(TrackCategory::State),
        "signals" => Ok(TrackCategory::Signals),
        "physics" => Ok(TrackCategory::Physics),
        "all" => Ok(TrackCategory::All),
        other => Err(McpError::invalid_params(
            format!("Unknown track category '{other}'. Valid: position, state, signals, physics, all"),
            None,
        )),
    }
}

pub async fn handle_spatial_watch(
    params: SpatialWatchParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    match params.action.as_str() {
        "add" => {
            let spec = params.watch.ok_or_else(|| {
                McpError::invalid_params("'watch' specification is required for add action", None)
            })?;

            let conditions: Vec<WatchCondition> = spec
                .conditions
                .iter()
                .map(|c| {
                    Ok(WatchCondition {
                        property: c.property.clone(),
                        operator: parse_operator(&c.operator)?,
                        value: c.value.clone(),
                    })
                })
                .collect::<Result<Vec<_>, McpError>>()?;

            let track: Vec<TrackCategory> = spec
                .track
                .iter()
                .map(|t| parse_track(t))
                .collect::<Result<Vec<_>, McpError>>()?;

            let watch = {
                let mut s = state.lock().await;
                s.watch_engine.add(spec.node, conditions, track)
            };

            let conditions_desc = if watch.conditions.is_empty() {
                "none".to_string()
            } else {
                watch
                    .conditions
                    .iter()
                    .map(|c| {
                        let val = c.value.as_ref().map(|v| v.to_string()).unwrap_or_default();
                        format!("{} {:?} {}", c.property, c.operator, val)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            let mut response = serde_json::json!({
                "watch_id": watch.id,
                "watching": watch.node,
                "conditions": conditions_desc,
                "tracking": watch.track,
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        "remove" => {
            let watch_id = params.watch_id.ok_or_else(|| {
                McpError::invalid_params("'watch_id' is required for remove action", None)
            })?;

            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.remove(&watch_id)
            };

            let mut response = serde_json::json!({
                "result": if removed { "ok" } else { "not_found" },
                "removed": if removed { 1 } else { 0 },
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        "list" => {
            let watches = {
                let s = state.lock().await;
                s.watch_engine
                    .list()
                    .iter()
                    .map(|w| {
                        let conditions_desc = if w.conditions.is_empty() {
                            "none".to_string()
                        } else {
                            w.conditions
                                .iter()
                                .map(|c| {
                                    let val = c.value.as_ref().map(|v| v.to_string()).unwrap_or_default();
                                    format!("{} {:?} {}", c.property, c.operator, val)
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        serde_json::json!({
                            "id": w.id,
                            "node": w.node,
                            "conditions": conditions_desc,
                            "tracking": w.track,
                        })
                    })
                    .collect::<Vec<_>>()
            };

            let mut response = serde_json::json!({
                "watches": watches,
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        "clear" => {
            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.clear()
            };

            let mut response = serde_json::json!({
                "result": "ok",
                "removed": removed,
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        other => Err(McpError::invalid_params(
            format!("Unknown action '{other}'. Valid: add, remove, list, clear"),
            None,
        )),
    }
}
```

Register in `mod.rs`:

```rust
pub mod watch;

use watch::SpatialWatchParams;
```

Add to the `#[tool_router]` impl:

```rust
/// Subscribe to changes on nodes or groups with optional conditions.
/// Watch triggers appear in spatial_delta responses.
#[tool(description = "Subscribe to changes on nodes or groups. Actions: 'add' (subscribe with optional conditions like health < 20), 'remove' (by watch_id), 'list' (show active watches), 'clear' (remove all). Watch triggers appear in spatial_delta responses under 'watch_triggers'.")]
pub async fn spatial_watch(
    &self,
    Parameters(params): Parameters<SpatialWatchParams>,
) -> Result<String, McpError> {
    watch::handle_spatial_watch(params, &self.state).await
}
```

**Acceptance Criteria:**
- [ ] `spatial_watch(action: "add", watch: { node: "enemies/scout_02" })` returns watch_id
- [ ] `spatial_watch(action: "add", watch: { node: "group:enemies", conditions: [{ property: "health", operator: "lt", value: 20 }] })` creates a conditional watch
- [ ] `spatial_watch(action: "list")` returns all active watches
- [ ] `spatial_watch(action: "remove", watch_id: "w_001")` removes the watch
- [ ] `spatial_watch(action: "clear")` removes all watches
- [ ] Budget block on all responses

---

### Unit 8: Wire `return_delta` on `spatial_action` (`stage-server`)

**File:** `crates/stage-server/src/mcp/mod.rs` (edit the `spatial_action` handler)

Replace the M4 placeholder in the `spatial_action` handler with real delta computation:

```rust
pub async fn spatial_action(
    &self,
    Parameters(params): Parameters<SpatialActionParams>,
) -> Result<String, McpError> {
    let action_request = build_action_request(&params)?;
    let data = query_addon(
        &self.state,
        "execute_action",
        serialize_params(&action_request)?,
    )
    .await?;

    let mut response: serde_json::Value = data;

    let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
    let used = stage_core::budget::estimate_tokens(json_bytes);
    inject_budget(&mut response, used, 500);

    if params.return_delta {
        // Compute a delta by fetching current state and diffing against baseline
        let has_baseline = {
            let s = self.state.lock().await;
            s.delta_engine.has_baseline()
        };

        if has_baseline {
            // Fetch current snapshot from addon
            let query_params = stage_protocol::query::GetSnapshotDataParams {
                perspective: stage_protocol::query::PerspectiveParam::Camera,
                radius: 50.0,
                include_offscreen: true,
                groups: vec![],
                class_filter: vec![],
                detail: stage_protocol::query::DetailLevel::Standard,
            };

            if let Ok(snap_data) = query_addon(
                &self.state,
                "get_snapshot_data",
                serialize_params(&query_params)?,
            )
            .await
            {
                if let Ok(raw_data) = serde_json::from_value::<stage_protocol::query::SnapshotResponse>(snap_data) {
                    let current_snapshots: Vec<stage_core::delta::EntitySnapshot> = raw_data
                        .entities
                        .iter()
                        .map(snapshot::to_entity_snapshot)
                        .collect();

                    let mut s = self.state.lock().await;
                    let delta = s.delta_engine.compute_delta(&current_snapshots, raw_data.frame);
                    let triggers = s.watch_engine.evaluate(
                        s.delta_engine.last_snapshot_map(),
                        &current_snapshots,
                        raw_data.frame,
                    );

                    // Update baseline
                    s.delta_engine.store_snapshot(raw_data.frame, current_snapshots);

                    // Build inline delta
                    let mut delta_json = serde_json::json!({
                        "from_frame": delta.from_frame,
                        "to_frame": delta.to_frame,
                    });
                    if let serde_json::Value::Object(ref mut map) = delta_json {
                        if !delta.moved.is_empty() {
                            map.insert("moved".into(), serde_json::to_value(&delta.moved).unwrap_or_default());
                        }
                        if !delta.state_changed.is_empty() {
                            map.insert("state_changed".into(), serde_json::to_value(&delta.state_changed).unwrap_or_default());
                        }
                        if !delta.entered.is_empty() {
                            map.insert("entered".into(), serde_json::to_value(&delta.entered).unwrap_or_default());
                        }
                        if !delta.exited.is_empty() {
                            map.insert("exited".into(), serde_json::to_value(&delta.exited).unwrap_or_default());
                        }
                        if !triggers.is_empty() {
                            map.insert("watch_triggers".into(), serde_json::to_value(&triggers).unwrap_or_default());
                        }
                    }

                    if let serde_json::Value::Object(ref mut map) = response {
                        map.insert("delta".into(), delta_json);
                    }
                }
            }
        } else {
            // No baseline — can't compute delta
            if let serde_json::Value::Object(ref mut map) = response {
                map.insert("delta".into(), serde_json::json!(null));
                map.insert(
                    "delta_note".into(),
                    serde_json::json!(
                        "No baseline snapshot. Call spatial_snapshot first, \
                         then use return_delta on actions."
                    ),
                );
            }
        }
    }

    serialize_response(&response)
}
```

**Implementation Notes:**
- `return_delta` does a full round-trip: action → fetch current state → diff against baseline → return inline
- If no baseline exists (no prior snapshot), returns `delta: null` with a note
- The delta is computed using the same engine as `spatial_delta`, ensuring consistency
- The inline delta also evaluates watches, so watch triggers appear in action responses too

**Acceptance Criteria:**
- [ ] `spatial_action(..., return_delta: true)` includes inline `delta` block
- [ ] Inline delta shows entities that moved due to the action
- [ ] If no baseline exists, `delta` is null with helpful note
- [ ] Watch triggers appear in the inline delta
- [ ] `return_delta: false` (default) does not fetch extra data

---

### Unit 9: Delta Engine `last_snapshot_map` Accessor

**File:** `crates/stage-core/src/delta.rs` (edit)

Add a public accessor for the watch engine to read the previous state:

```rust
impl DeltaEngine {
    /// Returns a reference to the stored snapshot map.
    /// Used by the watch engine to check "changed" conditions.
    pub fn last_snapshot_map(&self) -> &HashMap<String, EntitySnapshot> {
        &self.last_snapshot
    }
}
```

**Acceptance Criteria:**
- [ ] Watch engine can access previous entity state for "changed" operator evaluation

---

### Unit 10: Push Event Handling for Signal Subscriptions

**File:** `crates/stage-protocol/src/query.rs` (edit — append)

Add protocol types for signal subscription:

```rust
/// Request to subscribe to signal emissions on a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeSignalParams {
    pub path: String,
    pub signal: String,
}

/// Request to unsubscribe from signal emissions on a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeSignalParams {
    pub path: String,
    pub signal: String,
}
```

**File:** `crates/stage-server/src/tcp.rs` (edit)

In the read loop, handle `Event` messages by pushing them into the delta engine:

```rust
Ok(Message::Event { event, data }) => {
    if event == "signal_emitted" {
        let mut s = state.lock().await;
        if let (Some(node), Some(signal), Some(frame)) = (
            data.get("node").and_then(|v| v.as_str()),
            data.get("signal").and_then(|v| v.as_str()),
            data.get("frame").and_then(|v| v.as_u64()),
        ) {
            s.delta_engine.push_event(stage_core::delta::BufferedEvent {
                event_type: stage_core::delta::BufferedEventType::SignalEmitted,
                path: node.to_string(),
                frame,
                data: serde_json::json!({
                    "signal": signal,
                    "args": data.get("args").cloned().unwrap_or(serde_json::json!([])),
                }),
            });
        }
    } else {
        tracing::debug!("Received event: {event}");
    }
}
```

**Implementation Notes:**
- Signal subscription (`subscribe_signal`/`unsubscribe_signal`) is handled on the addon side — for M4, we handle the push events that arrive, but don't implement the subscribe/unsubscribe commands yet. The addon doesn't currently support signal subscriptions, so this unit is **future-ready plumbing only**.
- The delta engine's `event_buffer` is drained during `compute_delta` and included in the `signals_emitted` section of the response.
- The `Event` message type already exists in `stage-protocol/messages.rs` — we just need to handle it in the read loop.

**Acceptance Criteria:**
- [ ] `Event` messages from addon are parsed and stored in delta engine
- [ ] Protocol types for subscribe/unsubscribe exist (addon-side implementation deferred)
- [ ] Event buffer is consumed during delta computation

---

### Unit 11: Include Buffered Events in Delta Response

**File:** `crates/stage-server/src/mcp/delta.rs` (edit)

After computing the delta and before building the response, drain events from the delta engine and include them:

In `handle_spatial_delta`, between computing delta and building the response:

```rust
// 5b. Drain buffered events (signal emissions from push events)
let signals_emitted = {
    let mut s = state.lock().await;
    s.delta_engine.drain_events()
};

// In the response builder:
if !signals_emitted.is_empty() {
    let signal_entries: Vec<serde_json::Value> = signals_emitted
        .iter()
        .filter(|e| matches!(e.event_type, stage_core::delta::BufferedEventType::SignalEmitted))
        .map(|e| {
            serde_json::json!({
                "path": e.path,
                "signal": e.data.get("signal").unwrap_or(&serde_json::json!("unknown")),
                "args": e.data.get("args").unwrap_or(&serde_json::json!([])),
                "frame": e.frame,
            })
        })
        .collect();

    if !signal_entries.is_empty() {
        if let serde_json::Value::Object(ref mut map) = response {
            map.insert("signals_emitted".into(), serde_json::json!(signal_entries));
        }
    }
}
```

**Acceptance Criteria:**
- [ ] Buffered signal events appear in delta response as `signals_emitted`
- [ ] Events are cleared from buffer after being included in response

---

## Implementation Order

1. **Unit 1**: Delta engine (`stage-core/delta.rs`) — pure logic, no deps on other units
2. **Unit 2**: Watch engine (`stage-core/watch.rs`) — depends on Unit 1 for `values_equal`
3. **Unit 3**: Wire modules into `stage-core/lib.rs`
4. **Unit 9**: Add `last_snapshot_map` accessor to delta engine
5. **Unit 4**: Add engines to `SessionState` — depends on Units 1-3
6. **Unit 5**: Store snapshot in delta engine from `spatial_snapshot` — depends on Unit 4
7. **Unit 10**: Protocol types for signal subscription + push event handling
8. **Unit 6**: `spatial_delta` MCP tool — depends on Units 4-5
9. **Unit 7**: `spatial_watch` MCP tool — depends on Unit 4
10. **Unit 11**: Include buffered events in delta response — depends on Units 6, 10
11. **Unit 8**: Wire `return_delta` on `spatial_action` — depends on Units 5-6

---

## Testing

### Unit Tests: `crates/stage-core/src/delta.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn entity(path: &str, pos: Position3, state: &[(&str, serde_json::Value)]) -> EntitySnapshot {
        EntitySnapshot {
            path: path.to_string(),
            class: "CharacterBody3D".to_string(),
            position: pos,
            rotation_deg: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            groups: vec!["enemies".to_string()],
            state: state.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            visible: true,
        }
    }

    #[test]
    fn detect_movement_above_threshold() { /* entity moved 1.0 unit → MovedEntity */ }

    #[test]
    fn suppress_movement_below_threshold() { /* entity moved 0.005 units → None */ }

    #[test]
    fn detect_state_change() { /* health: 100 → 80 → StateChange */ }

    #[test]
    fn suppress_float_change_below_threshold() { /* 1.0 → 1.0005 → no change */ }

    #[test]
    fn detect_entered_entity() { /* new entity not in baseline → EnteredEntity */ }

    #[test]
    fn detect_exited_entity() { /* entity in baseline not in current → ExitedEntity */ }

    #[test]
    fn store_and_compute_full_cycle() {
        /* store baseline → modify entities → compute_delta → verify all categories */
    }

    #[test]
    fn event_buffer_drain() {
        /* push events → drain → verify empty after drain */
    }

    #[test]
    fn values_equal_float_threshold() {
        /* 1.0 vs 1.0005 → equal; 1.0 vs 1.002 → not equal */
    }

    #[test]
    fn values_equal_string() {
        /* "patrol" vs "alert" → not equal */
    }
}
```

### Unit Tests: `crates/stage-core/src/watch.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_entity(path: &str, health: f64) -> EntitySnapshot { /* helper */ }

    #[test]
    fn add_and_list_watch() { /* add → list → verify present */ }

    #[test]
    fn remove_watch() { /* add → remove → list → verify gone */ }

    #[test]
    fn clear_all_watches() { /* add 3 → clear → verify 0, returns 3 */ }

    #[test]
    fn evaluate_lt_condition_triggers() {
        /* health=15, condition: health < 20 → trigger */
    }

    #[test]
    fn evaluate_lt_condition_no_trigger() {
        /* health=80, condition: health < 20 → no trigger */
    }

    #[test]
    fn evaluate_gt_condition() { /* speed=12, condition: speed > 10 → trigger */ }

    #[test]
    fn evaluate_eq_condition() { /* state="alert", condition: state eq "alert" → trigger */ }

    #[test]
    fn evaluate_changed_condition() {
        /* prev health=80, curr health=15 → trigger with description */
    }

    #[test]
    fn group_watch_matches_all_members() {
        /* watch "group:enemies" → matches all entities in enemies group */
    }

    #[test]
    fn node_watch_matches_exact_path() {
        /* watch "enemies/scout_02" → matches only that entity */
    }

    #[test]
    fn watched_paths_expands_groups() {
        /* watch "group:enemies" with 3 enemies → returns 3 paths */
    }
}
```

### Integration-level Tests: `crates/stage-server/src/mcp/delta.rs`

These test the MCP parameter parsing and response building. Since the actual addon call requires a running Godot instance, these test the param validation and error paths:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_params_defaults() {
        let json = r#"{ }"#;
        let params: SpatialDeltaParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.perspective, "camera");
        assert_eq!(params.radius, 50.0);
        assert!(params.since_frame.is_none());
    }
}
```

### Integration-level Tests: `crates/stage-server/src/mcp/watch.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_operator_valid() {
        assert!(matches!(parse_operator("lt"), Ok(ConditionOperator::Lt)));
        assert!(matches!(parse_operator("gt"), Ok(ConditionOperator::Gt)));
        assert!(matches!(parse_operator("eq"), Ok(ConditionOperator::Eq)));
        assert!(matches!(parse_operator("changed"), Ok(ConditionOperator::Changed)));
    }

    #[test]
    fn parse_operator_invalid() {
        assert!(parse_operator("invalid").is_err());
    }

    #[test]
    fn parse_track_valid() {
        assert!(matches!(parse_track("all"), Ok(TrackCategory::All)));
        assert!(matches!(parse_track("position"), Ok(TrackCategory::Position)));
    }

    #[test]
    fn parse_track_invalid() {
        assert!(parse_track("everything").is_err());
    }
}
```

---

## Verification Checklist

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Verify new modules compile
cargo test -p stage-core -- delta
cargo test -p stage-core -- watch

# Verify MCP tool registration (check tool count)
# The server should register 7 tools: spatial_snapshot, spatial_inspect,
# scene_tree, spatial_action, spatial_query, spatial_delta, spatial_watch
```
