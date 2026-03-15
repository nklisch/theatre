# Design: Milestone 3 — Actions & Queries

## Overview

M3 delivers two MCP tools: `spatial_action` for debugging-oriented game state manipulation, and `spatial_query` for targeted spatial questions. These make the agent an active debugger, not just an observer.

**Exit Criteria:** Agent pauses game, teleports enemy to wall, advances 5 frames, takes a snapshot — sees enemy stopped at wall. Agent raycasts from enemy to player, gets obstruction info. Agent queries nearest 5 nodes to player with group filter. Agent calls `take_damage(50)` on a node and sees the result.

**Depends on:** M1 (TCP query flow, collector, MCP tool registration, spatial types, budget system)

---

## Implementation Units

### Unit 1: Protocol Types for Actions (`stage-protocol`)

**File:** `crates/stage-protocol/src/query.rs` (append to existing)

Add request/response types for all action operations. The addon receives these via TCP and executes them against the Godot scene tree.

```rust
// --- spatial_action protocol types ---

/// Parameters for action execution queries.
/// The server sends one of these per spatial_action MCP call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ActionRequest {
    Pause {
        paused: bool,
    },
    AdvanceFrames {
        frames: u32,
    },
    AdvanceTime {
        seconds: f64,
    },
    Teleport {
        path: String,
        position: Vec<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rotation_deg: Option<f64>,
    },
    SetProperty {
        path: String,
        property: String,
        value: serde_json::Value,
    },
    CallMethod {
        path: String,
        method: String,
        #[serde(default)]
        args: Vec<serde_json::Value>,
    },
    EmitSignal {
        path: String,
        signal: String,
        #[serde(default)]
        args: Vec<serde_json::Value>,
    },
    SpawnNode {
        scene_path: String,
        parent: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        position: Option<Vec<f64>>,
    },
    RemoveNode {
        path: String,
    },
}

/// Response from action execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResponse {
    /// Which action was performed.
    pub action: String,
    /// "ok" or error description.
    pub result: String,
    /// Action-specific details (previous values, new values, etc.).
    pub details: serde_json::Map<String, serde_json::Value>,
    /// Frame number after action completed.
    pub frame: u64,
}
```

**Implementation Notes:**
- `ActionRequest` uses serde's internally-tagged enum (`#[serde(tag = "action")]`) so the JSON has `{ "action": "teleport", "path": "...", "position": [...] }` — matches CONTRACT.md's flat structure.
- `ActionResponse.details` is a free-form map because each action returns different data (teleport returns positions, set_property returns old/new values, etc.).
- The addon receives the `ActionRequest` as the `params` field of a `Message::Query` with `method: "execute_action"`.

**Acceptance Criteria:**
- [ ] `ActionRequest` serde round-trip for all 9 variants
- [ ] Tagged enum serializes with `"action"` field in snake_case
- [ ] `ActionResponse` serializes with action, result, details, frame fields
- [ ] `cargo test -p stage-protocol` passes

---

### Unit 2: Protocol Types for Spatial Queries (`stage-protocol`)

**File:** `crates/stage-protocol/src/query.rs` (append to existing)

Add request/response types for targeted spatial queries that the addon must execute (raycast, nav path) or provide raw data for (nearest, radius — server computes from index).

```rust
// --- spatial_query protocol types ---

/// Parameters for spatial queries executed by the addon.
/// Only query types requiring Godot engine access go through TCP.
/// nearest/radius/area are handled server-side from the spatial index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "query_type", rename_all = "snake_case")]
pub enum SpatialQueryRequest {
    /// Physics raycast between two points/nodes.
    Raycast {
        from: QueryOrigin,
        to: QueryOrigin,
        #[serde(default)]
        collision_mask: Option<u32>,
    },
    /// Navigation mesh path distance.
    PathDistance {
        from: QueryOrigin,
        to: QueryOrigin,
    },
    /// Get position and forward vector for a node (for server-side queries).
    ResolveNode {
        path: String,
    },
}

/// Origin for a spatial query — either a node path or a world position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryOrigin {
    /// A world-space coordinate.
    Position(Vec<f64>),
    /// A node path (server resolves to position via addon).
    Node(String),
}

/// Response for raycast query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaycastResponse {
    /// True if the ray reached the target unobstructed.
    pub clear: bool,
    /// Node that blocked the ray (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_by: Option<String>,
    /// World position where the ray was blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_at: Option<Vec<f64>>,
    /// Total distance from source to target.
    pub total_distance: f64,
    /// Distance from source to the hit point (or total if clear).
    pub clear_distance: f64,
}

/// Response for navigation path distance query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavPathResponse {
    /// Navigation mesh distance.
    pub nav_distance: f64,
    /// Straight-line distance for comparison.
    pub straight_distance: f64,
    /// Ratio of nav_distance / straight_distance.
    pub path_ratio: f64,
    /// Number of waypoints in the path.
    pub path_points: u32,
    /// Whether a path was found.
    pub traversable: bool,
}

/// Response for resolving a node to its position and forward vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveNodeResponse {
    pub position: Vec<f64>,
    pub forward: Vec<f64>,
    pub groups: Vec<String>,
}
```

**Implementation Notes:**
- `QueryOrigin` uses `#[serde(untagged)]` so it accepts either `"player"` (string → Node) or `[1.0, 2.0, 3.0]` (array → Position). The `Position` variant must come first so array literals match before falling through to string.
- `nearest`, `radius`, and `area` queries don't need addon involvement — the server computes them from its spatial index (rebuilt from the last snapshot). The server only needs to resolve node paths to positions.
- `relationship` query is composed server-side from a raycast + spatial index lookups + optional nav path.
- `ResolveNode` is a lightweight query to get a node's position when the server needs it for index-based queries.

**Acceptance Criteria:**
- [ ] `SpatialQueryRequest` serde round-trip for all 3 variants
- [ ] `QueryOrigin` deserializes both `"player"` and `[1.0, 2.0, 3.0]` correctly
- [ ] `RaycastResponse`, `NavPathResponse`, `ResolveNodeResponse` round-trip
- [ ] `cargo test -p stage-protocol` passes

---

### Unit 3: Spatial Index (`stage-core`)

**File:** `crates/stage-core/src/index.rs` (new file)

The spatial index enables efficient nearest-neighbor and radius queries server-side, without re-querying the addon. Uses rstar R-tree for 3D.

```rust
use rstar::{RTree, RTreeObject, AABB, PointDistance};
use crate::types::Position3;

/// An indexed entity with its position and metadata.
#[derive(Debug, Clone)]
pub struct IndexedEntity {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub groups: Vec<String>,
}

impl RTreeObject for IndexedEntity {
    type Envelope = AABB<[f64; 3]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.position)
    }
}

impl PointDistance for IndexedEntity {
    fn distance_2(&self, point: &[f64; 3]) -> f64 {
        let dx = self.position[0] - point[0];
        let dy = self.position[1] - point[1];
        let dz = self.position[2] - point[2];
        dx * dx + dy * dy + dz * dz
    }
}

/// R-tree spatial index for efficient spatial queries.
pub struct SpatialIndex {
    tree: RTree<IndexedEntity>,
}

impl SpatialIndex {
    /// Build a new index from a set of entities.
    pub fn build(entities: Vec<IndexedEntity>) -> Self {
        Self {
            tree: RTree::bulk_load(entities),
        }
    }

    /// Create an empty index.
    pub fn empty() -> Self {
        Self {
            tree: RTree::new(),
        }
    }

    /// Find the K nearest entities to a point.
    /// Results are sorted by distance (nearest first).
    /// Applies optional group and class filters.
    pub fn nearest(
        &self,
        point: Position3,
        k: usize,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        self.tree
            .nearest_neighbor_iter(&point)
            .filter(|e| Self::matches_filters(e, groups, class_filter))
            .take(k)
            .map(|e| NearestResult {
                path: e.path.clone(),
                class: e.class.clone(),
                position: e.position,
                distance: crate::bearing::distance(point, e.position),
                groups: e.groups.clone(),
            })
            .collect()
    }

    /// Find all entities within a radius of a point.
    /// Results are sorted by distance (nearest first).
    pub fn within_radius(
        &self,
        point: Position3,
        radius: f64,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        let r2 = radius * radius;
        let envelope = AABB::from_corners(
            [point[0] - radius, point[1] - radius, point[2] - radius],
            [point[0] + radius, point[1] + radius, point[2] + radius],
        );
        let mut results: Vec<_> = self.tree
            .locate_in_envelope(&envelope)
            .filter(|e| e.distance_2(&point) <= r2)
            .filter(|e| Self::matches_filters(e, groups, class_filter))
            .map(|e| NearestResult {
                path: e.path.clone(),
                class: e.class.clone(),
                position: e.position,
                distance: crate::bearing::distance(point, e.position),
                groups: e.groups.clone(),
            })
            .collect();
        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Check if an entity matches group and class filters.
    fn matches_filters(entity: &IndexedEntity, groups: &[String], class_filter: &[String]) -> bool {
        let group_ok = groups.is_empty()
            || entity.groups.iter().any(|g| groups.contains(g));
        let class_ok = class_filter.is_empty()
            || class_filter.iter().any(|c| c == &entity.class);
        group_ok && class_ok
    }

    /// Return the number of indexed entities.
    pub fn len(&self) -> usize {
        self.tree.size()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }
}

/// Result of a nearest/radius query.
#[derive(Debug, Clone)]
pub struct NearestResult {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub distance: f64,
    pub groups: Vec<String>,
}
```

**Implementation Notes:**
- `rstar::RTree::bulk_load` is O(n log n) and produces a well-balanced tree. For typical game scenes (<500 dynamic entities), build time is sub-millisecond.
- `nearest_neighbor_iter` returns an iterator sorted by distance — ideal for K-nearest since we just `.take(k)`.
- `within_radius` uses AABB envelope filtering first (coarse), then exact distance check (fine) — standard rstar pattern.
- The spatial index is rebuilt after every `spatial_snapshot` call (the server already has all positions). Between snapshots, it may be stale — acceptable for M3 queries.

**Acceptance Criteria:**
- [ ] `SpatialIndex::build` constructs from Vec<IndexedEntity>
- [ ] `nearest(point, k)` returns K nearest entities sorted by distance
- [ ] `within_radius(point, r)` returns all entities within radius, sorted
- [ ] Group and class filters exclude non-matching entities
- [ ] Empty index returns empty results (no panics)
- [ ] `cargo test -p stage-core` passes with new index tests

---

### Unit 4: Index Integration in Server Session State

**File:** `crates/stage-server/src/tcp.rs` (modify existing `SessionState`)

Add the spatial index to session state and rebuild it after each snapshot query.

```rust
// Add to existing SessionState struct:
use stage_core::index::SpatialIndex;

pub struct SessionState {
    // ... existing fields ...
    /// Spatial index built from the most recent snapshot.
    pub spatial_index: SpatialIndex,
}

// In Default impl or constructor:
impl Default for SessionState {
    fn default() -> Self {
        Self {
            // ... existing fields ...
            spatial_index: SpatialIndex::empty(),
        }
    }
}
```

**File:** `crates/stage-server/src/mcp/mod.rs` (modify `spatial_snapshot`)

After processing a snapshot response, rebuild the spatial index:

```rust
// After step 6 (sort by distance), before step 7 (resolve budget):
// Rebuild spatial index from snapshot data
{
    let indexed: Vec<IndexedEntity> = raw_data.entities.iter().map(|e| {
        IndexedEntity {
            path: e.path.clone(),
            class: e.class.clone(),
            position: vec_to_array3(&e.position),
            groups: e.groups.clone(),
        }
    }).collect();
    let mut state = self.state.lock().await;
    state.spatial_index = SpatialIndex::build(indexed);
}
```

**Implementation Notes:**
- The index is rebuilt on every `spatial_snapshot` call. This is cheap (<1ms for 500 entities) and keeps the index fresh.
- The lock is held only during the index swap — not during the TCP query or response building.
- For `spatial_query`, the handler reads the index from session state.

**Acceptance Criteria:**
- [ ] `SessionState` includes `spatial_index: SpatialIndex` field
- [ ] `spatial_snapshot` rebuilds the index after receiving addon data
- [ ] Index is accessible from other tool handlers via `self.state`

---

### Unit 5: Action Handler in GDExtension (`stage-godot`)

**File:** `crates/stage-godot/src/action_handler.rs` (new file)

Executes action requests against the live Godot scene tree. Each action variant maps to Godot API calls.

```rust
use godot::prelude::*;
use godot::classes::{Engine, SceneTree, PackedScene, ResourceLoader};
use stage_protocol::query::{ActionRequest, ActionResponse};
use crate::collector::StageCollector;

/// Execute an action against the Godot scene tree.
/// Called from the query handler on the main thread.
pub fn execute_action(
    request: &ActionRequest,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    match request {
        ActionRequest::Pause { paused } => execute_pause(*paused, collector),
        ActionRequest::AdvanceFrames { frames } => execute_advance_frames(*frames, collector),
        ActionRequest::AdvanceTime { seconds } => execute_advance_time(*seconds, collector),
        ActionRequest::Teleport { path, position, rotation_deg } => {
            execute_teleport(path, position, *rotation_deg, collector)
        }
        ActionRequest::SetProperty { path, property, value } => {
            execute_set_property(path, property, value, collector)
        }
        ActionRequest::CallMethod { path, method, args } => {
            execute_call_method(path, method, args, collector)
        }
        ActionRequest::EmitSignal { path, signal, args } => {
            execute_emit_signal(path, signal, args, collector)
        }
        ActionRequest::SpawnNode { scene_path, parent, name, position } => {
            execute_spawn_node(scene_path, parent, name.as_deref(), position.as_deref(), collector)
        }
        ActionRequest::RemoveNode { path } => execute_remove_node(path, collector),
    }
}

fn get_frame(collector: &StageCollector) -> u64 {
    collector.get_frame_info().frame
}

fn resolve_node(collector: &StageCollector, path: &str) -> Result<Gd<Node>, String> {
    // Reuse collector's existing resolve_node
    collector.resolve_node_public(path)
}

fn execute_pause(paused: bool, collector: &StageCollector) -> Result<ActionResponse, String> {
    let tree = collector.base().get_tree()
        .ok_or("Not in scene tree")?;
    tree.set_pause(paused);
    Ok(ActionResponse {
        action: "pause".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("paused".into(), serde_json::Value::Bool(paused)),
        ]),
        frame: get_frame(collector),
    })
}

fn execute_advance_frames(
    frames: u32,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    // Verify the tree is paused
    let tree = collector.base().get_tree()
        .ok_or("Not in scene tree")?;
    if !tree.is_paused() {
        return Err("Cannot advance frames: scene tree is not paused. Use pause first.".into());
    }

    // Advance by unpausing, ticking, then re-pausing.
    // Each "advance" unpause → run N physics frames → re-pause.
    // We use MainLoop::physics_frame signal approach, but for simplicity
    // we temporarily unpause the tree, let the engine tick, then re-pause.
    // In practice, this is handled by setting a counter and using _physics_process.
    let physics_ticks = Engine::singleton().get_physics_ticks_per_second() as u32;
    // Store the advance request — the runtime.gd poll loop will handle stepping
    // For M3, we use the simpler approach: directly manipulate SceneTree.
    // Note: advance_frames is inherently complex in Godot because we can't
    // force physics ticks from GDExtension synchronously. We set an internal
    // counter and return after completion.

    // Simplified approach for M3: temporarily unpause for N frames.
    // The action_handler stores the pending advance, and tcp_server.poll()
    // drives it by unpausing/re-pausing across physics frames.

    Ok(ActionResponse {
        action: "advance_frames".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("frames_requested".into(), serde_json::json!(frames)),
        ]),
        frame: get_frame(collector),
    })
}

fn execute_advance_time(
    seconds: f64,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let tree = collector.base().get_tree()
        .ok_or("Not in scene tree")?;
    if !tree.is_paused() {
        return Err("Cannot advance time: scene tree is not paused. Use pause first.".into());
    }
    let tps = Engine::singleton().get_physics_ticks_per_second() as f64;
    let frames = (seconds * tps).round() as u32;
    // Delegate to advance_frames logic
    execute_advance_frames(frames, collector)
        .map(|mut r| {
            r.action = "advance_time".into();
            r.details.insert("seconds".into(), serde_json::json!(seconds));
            r
        })
}

fn execute_teleport(
    path: &str,
    position: &[f64],
    rotation_deg: Option<f64>,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let node = resolve_node(collector, path)?;

    let mut details = serde_json::Map::new();

    if let Ok(mut node3d) = node.clone().try_cast::<Node3D>() {
        let prev = node3d.get_global_position();
        details.insert("previous_position".into(), serde_json::json!([prev.x, prev.y, prev.z]));

        let new_pos = match position.len() {
            3 => Vector3::new(position[0] as f32, position[1] as f32, position[2] as f32),
            2 => Vector3::new(position[0] as f32, 0.0, position[1] as f32),
            _ => return Err(format!("Invalid position: expected 2 or 3 components, got {}", position.len())),
        };
        node3d.set_global_position(new_pos);
        details.insert("new_position".into(), serde_json::json!([new_pos.x, new_pos.y, new_pos.z]));

        if let Some(rot) = rotation_deg {
            let mut euler = node3d.get_rotation_degrees();
            euler.y = rot as f32;
            node3d.set_rotation_degrees(euler);
            details.insert("rotation_deg".into(), serde_json::json!(rot));
        }
    } else if let Ok(mut node2d) = node.try_cast::<Node2D>() {
        let prev = node2d.get_global_position();
        details.insert("previous_position".into(), serde_json::json!([prev.x, prev.y]));

        let new_pos = match position.len() {
            2 => Vector2::new(position[0] as f32, position[1] as f32),
            _ => return Err(format!("Invalid 2D position: expected 2 components, got {}", position.len())),
        };
        node2d.set_global_position(new_pos);
        details.insert("new_position".into(), serde_json::json!([new_pos.x, new_pos.y]));

        if let Some(rot) = rotation_deg {
            node2d.set_rotation_degrees(rot as f32);
            details.insert("rotation_deg".into(), serde_json::json!(rot));
        }
    } else {
        return Err(format!("Node '{path}' is not a Node3D or Node2D — cannot teleport"));
    }

    Ok(ActionResponse {
        action: "teleport".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_set_property(
    path: &str,
    property: &str,
    value: &serde_json::Value,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;
    let obj = node.upcast_mut::<Object>();

    // Read previous value
    let prev_variant = obj.get(property.into());
    let prev_json = crate::collector::variant_to_json(&prev_variant);

    // Convert JSON value to Variant
    let new_variant = json_to_variant(value)?;
    obj.set(property.into(), &new_variant);

    let mut details = serde_json::Map::new();
    details.insert("property".into(), serde_json::json!(property));
    if let Some(prev) = prev_json {
        details.insert("previous_value".into(), prev);
    }
    details.insert("new_value".into(), value.clone());

    Ok(ActionResponse {
        action: "set_property".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_call_method(
    path: &str,
    method: &str,
    args: &[serde_json::Value],
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;

    // Check method exists
    if !node.has_method(method.into()) {
        return Err(format!("Method '{method}' not found on node '{path}'"));
    }

    let variant_args: Vec<Variant> = args.iter()
        .map(|a| json_to_variant(a))
        .collect::<Result<_, _>>()?;

    let result = node.callv(method.into(), &variant_args.into());
    let result_json = crate::collector::variant_to_json(&result);

    let mut details = serde_json::Map::new();
    details.insert("method".into(), serde_json::json!(method));
    if let Some(rv) = result_json {
        details.insert("return_value".into(), rv);
    } else {
        details.insert("return_value".into(), serde_json::Value::Null);
    }

    Ok(ActionResponse {
        action: "call_method".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_emit_signal(
    path: &str,
    signal: &str,
    args: &[serde_json::Value],
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;

    let variant_args: Vec<Variant> = args.iter()
        .map(|a| json_to_variant(a))
        .collect::<Result<_, _>>()?;

    // emit_signal takes signal name + varargs
    node.emit_signal(signal.into(), &variant_args);

    Ok(ActionResponse {
        action: "emit_signal".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("signal".into(), serde_json::json!(signal)),
        ]),
        frame: get_frame(collector),
    })
}

fn execute_spawn_node(
    scene_path: &str,
    parent_path: &str,
    name: Option<&str>,
    position: Option<&[f64]>,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut parent = resolve_node(collector, parent_path)?;

    // Load the scene
    let mut loader = ResourceLoader::singleton();
    let resource = loader.load(scene_path.into())
        .ok_or_else(|| format!("Could not load scene: {scene_path}"))?;
    let packed = resource.try_cast::<PackedScene>()
        .map_err(|_| format!("Resource '{scene_path}' is not a PackedScene"))?;

    // Instantiate
    let mut instance = packed.instantiate()
        .ok_or_else(|| format!("Failed to instantiate scene: {scene_path}"))?;

    // Set name if provided
    if let Some(n) = name {
        instance.set_name(n.into());
    }

    // Set position if provided
    if let Some(pos) = position {
        if let Ok(mut n3d) = instance.clone().try_cast::<Node3D>() {
            if pos.len() >= 3 {
                n3d.set_global_position(Vector3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32));
            }
        } else if let Ok(mut n2d) = instance.clone().try_cast::<Node2D>() {
            if pos.len() >= 2 {
                n2d.set_global_position(Vector2::new(pos[0] as f32, pos[1] as f32));
            }
        }
    }

    let node_path = format!(
        "{}/{}",
        parent_path,
        instance.get_name()
    );

    parent.add_child(&instance);

    Ok(ActionResponse {
        action: "spawn_node".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("scene_path".into(), serde_json::json!(scene_path)),
            ("node_path".into(), serde_json::json!(node_path)),
        ]),
        frame: get_frame(collector),
    })
}

fn execute_remove_node(
    path: &str,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;
    let class = node.get_class().to_string();

    node.queue_free();

    Ok(ActionResponse {
        action: "remove_node".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("removed_path".into(), serde_json::json!(path)),
            ("removed_class".into(), serde_json::json!(class)),
        ]),
        frame: get_frame(collector),
    })
}

/// Convert a JSON value to a Godot Variant.
/// Supports: null, bool, int, float, string, array, object.
fn json_to_variant(value: &serde_json::Value) -> Result<Variant, String> {
    match value {
        serde_json::Value::Null => Ok(Variant::nil()),
        serde_json::Value::Bool(b) => Ok(b.to_variant()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_variant())
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_variant())
            } else {
                Err(format!("Unsupported number: {n}"))
            }
        }
        serde_json::Value::String(s) => Ok(GString::from(s.as_str()).to_variant()),
        serde_json::Value::Array(arr) => {
            // Try to detect Vector2/Vector3 (2-3 element numeric arrays)
            if arr.len() == 2 && arr.iter().all(|v| v.is_number()) {
                let x = arr[0].as_f64().unwrap_or(0.0) as f32;
                let y = arr[1].as_f64().unwrap_or(0.0) as f32;
                return Ok(Vector2::new(x, y).to_variant());
            }
            if arr.len() == 3 && arr.iter().all(|v| v.is_number()) {
                let x = arr[0].as_f64().unwrap_or(0.0) as f32;
                let y = arr[1].as_f64().unwrap_or(0.0) as f32;
                let z = arr[2].as_f64().unwrap_or(0.0) as f32;
                return Ok(Vector3::new(x, y, z).to_variant());
            }
            // Generic array
            let godot_array: VariantArray = arr.iter()
                .map(|v| json_to_variant(v))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .collect();
            Ok(godot_array.to_variant())
        }
        serde_json::Value::Object(map) => {
            let dict = Dictionary::new();
            for (k, v) in map {
                let key = GString::from(k.as_str()).to_variant();
                let val = json_to_variant(v)?;
                dict.clone().set(key, val);
            }
            Ok(dict.to_variant())
        }
    }
}
```

**Implementation Notes:**
- `resolve_node_public` is a new public wrapper around the existing private `resolve_node` in collector.rs. It needs to be exposed for action_handler.
- `json_to_variant` detects 2/3-element numeric arrays as Vector2/Vector3 — this matches Godot's convention and lets agents pass positions naturally as `[x, y, z]`.
- `execute_advance_frames` is the trickiest action. Since we can't synchronously advance physics frames from GDExtension, this is handled via a pending-advance mechanism (see Unit 6).
- `execute_spawn_node` calls `add_child` which adds the node to the tree immediately. Position is set before adding to tree so there's no flicker.
- All actions return the current frame number for delta correlation.

**Acceptance Criteria:**
- [ ] `execute_pause(true)` pauses the scene tree; `execute_pause(false)` unpauses
- [ ] `execute_teleport` moves Node3D to specified position, returns previous position
- [ ] `execute_set_property` changes property, returns old/new values
- [ ] `execute_call_method` calls method, returns result; errors if method not found
- [ ] `execute_emit_signal` emits signal on node
- [ ] `execute_spawn_node` instantiates scene, adds to parent, sets position
- [ ] `execute_remove_node` queue_frees the node
- [ ] `json_to_variant` correctly converts null, bool, int, float, string, Vector2, Vector3, array, dict

---

### Unit 6: Frame Advance Mechanism

**File:** `crates/stage-godot/src/tcp_server.rs` (modify existing)

Frame advance requires cooperation between the action handler and the physics tick loop. Since we can't force Godot to tick synchronously, we use a pending-advance pattern.

```rust
// Add to StageTCPServer fields:
pub struct StageTCPServer {
    // ... existing fields ...
    /// Number of physics frames remaining to advance.
    advance_remaining: u32,
    /// The request ID waiting for advance completion.
    advance_pending_id: Option<String>,
}

// In poll() method, add advance processing before try_read:
fn poll(&mut self) {
    // ... existing accept logic ...

    // Process frame advance if pending
    if self.advance_remaining > 0 {
        self.advance_remaining -= 1;
        if self.advance_remaining == 0 {
            // Re-pause the tree
            if let Some(tree) = self.base().get_tree() {
                tree.set_pause(true);
            }
            // Send the deferred response
            if let Some(id) = self.advance_pending_id.take() {
                let frame = self.collector.as_ref()
                    .map(|c| c.bind().get_frame_info().frame)
                    .unwrap_or(0);
                let response = ActionResponse {
                    action: "advance_frames".into(),
                    result: "ok".into(),
                    details: serde_json::Map::from_iter([
                        ("new_frame".into(), serde_json::json!(frame)),
                    ]),
                    frame,
                };
                let data = serde_json::to_value(&response).unwrap();
                self.send_response(Message::Response { id, data });
            }
        }
        return; // Don't process new queries while advancing
    }

    // ... existing try_read logic ...
}
```

**Implementation Notes:**
- When `advance_frames` or `advance_time` is requested, the action handler sets `advance_remaining` and `advance_pending_id` on the TCP server, then temporarily unpauses the tree.
- Each `poll()` call (once per `_physics_process`) decrements the counter. When it reaches 0, the tree is re-paused and the deferred response is sent.
- While advancing, new queries are deferred (the `return` skips `try_read`). This ensures the game ticks the requested number of frames without interference.
- The action handler needs a reference to the TCP server to set these fields. This is achieved by passing a mutable reference or using an `Arc<Mutex<AdvanceState>>` shared between the action handler and TCP server.

**Alternative approach (simpler for M3):** The action handler stores the advance request in a shared `AdvanceState` struct on the collector. The TCP server reads from it during `poll()`.

```rust
/// Shared advance state between action_handler and tcp_server.
pub struct AdvanceState {
    pub remaining: u32,
    pub pending_id: Option<String>,
}
```

Add `advance_state: RefCell<AdvanceState>` to `StageCollector` since both the action handler and TCP server have access to it.

**Acceptance Criteria:**
- [ ] `advance_frames(5)` causes exactly 5 physics ticks before re-pausing
- [ ] `advance_time(0.5)` converts to frames using physics tick rate
- [ ] Response is sent only after all frames have been processed
- [ ] New queries are deferred while advancing
- [ ] Tree is re-paused after advancing

---

### Unit 7: Spatial Query Handler in GDExtension (`stage-godot`)

**File:** `crates/stage-godot/src/collector.rs` (add methods to StageCollector)

Add raycast and navigation path methods to the collector.

```rust
// Add to StageCollector impl:

/// Perform a physics raycast from one point to another.
pub fn raycast(
    &self,
    from: Vector3,
    to: Vector3,
    collision_mask: Option<u32>,
) -> Result<RaycastResponse, String> {
    let tree = self.base().get_tree()
        .ok_or("Not in scene tree")?;
    let world = tree.get_root()
        .ok_or("No root")?
        .get_world_3d()
        .ok_or("No World3D — is this a 3D scene?")?;
    let space = world.get_space();
    let mut physics_server = PhysicsServer3D::singleton();
    let direct_state = physics_server.space_get_direct_state(space)
        .ok_or("Could not get physics direct state")?;

    let mut query = PhysicsRayQueryParameters3D::create(from, to).unwrap();
    if let Some(mask) = collision_mask {
        query.set_collision_mask(mask);
    }

    let result = direct_state.intersect_ray(&query);
    let total_distance = from.distance_to(to);

    if result.is_empty() {
        Ok(RaycastResponse {
            clear: true,
            blocked_by: None,
            blocked_at: None,
            total_distance: total_distance as f64,
            clear_distance: total_distance as f64,
        })
    } else {
        let hit_pos: Vector3 = result.get("position").unwrap_or(Variant::nil()).to();
        let collider: Option<Gd<Object>> = result.get("collider")
            .map(|v| v.to::<Gd<Object>>())
            .ok();
        let blocked_by = collider
            .and_then(|c| c.try_cast::<Node>().ok())
            .map(|n| self.get_relative_path(&n));

        Ok(RaycastResponse {
            clear: false,
            blocked_by,
            blocked_at: Some(vec![hit_pos.x as f64, hit_pos.y as f64, hit_pos.z as f64]),
            total_distance: total_distance as f64,
            clear_distance: from.distance_to(hit_pos) as f64,
        })
    }
}

/// Get navigation path distance between two points.
pub fn get_nav_path(
    &self,
    from: Vector3,
    to: Vector3,
) -> Result<NavPathResponse, String> {
    let nav_server = NavigationServer3D::singleton();

    // Get the default navigation map
    let maps = nav_server.get_maps();
    if maps.is_empty() {
        return Err("No navigation maps available. Is NavigationServer3D active?".into());
    }
    let map = maps.get(0);

    let path = nav_server.map_get_path(map, from, to, true);
    let traversable = path.len() > 0;
    let nav_distance: f64 = if traversable {
        let mut total = 0.0f32;
        for i in 1..path.len() {
            total += path[i - 1].distance_to(path[i]);
        }
        total as f64
    } else {
        0.0
    };

    let straight_distance = from.distance_to(to) as f64;

    Ok(NavPathResponse {
        nav_distance,
        straight_distance,
        path_ratio: if straight_distance > 0.0 { nav_distance / straight_distance } else { 1.0 },
        path_points: path.len() as u32,
        traversable,
    })
}

/// Resolve a node path to its position, forward vector, and groups.
pub fn resolve_node_position(&self, path: &str) -> Result<ResolveNodeResponse, String> {
    let node = self.resolve_node(path)?;
    if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
        let pos = n3d.get_global_position();
        let fwd = -n3d.get_global_basis().col_c(); // -Z is forward in Godot
        Ok(ResolveNodeResponse {
            position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
            forward: vec![fwd.x as f64, fwd.y as f64, fwd.z as f64],
            groups: self.get_groups(&node),
        })
    } else if let Ok(n2d) = node.clone().try_cast::<Node2D>() {
        let pos = n2d.get_global_position();
        Ok(ResolveNodeResponse {
            position: vec![pos.x as f64, pos.y as f64],
            forward: vec![1.0, 0.0], // 2D: rightward default
            groups: self.get_groups(&node),
        })
    } else {
        Err(format!("Node '{path}' is not a Node3D or Node2D"))
    }
}

/// Public wrapper for resolve_node (used by action_handler).
pub fn resolve_node_public(&self, path: &str) -> Result<Gd<Node>, String> {
    self.resolve_node(path)
}
```

**Implementation Notes:**
- `raycast` uses `PhysicsServer3D::space_get_direct_state` which provides synchronous raycast access on the main thread. This is safe during `_physics_process` which is where all queries execute.
- `get_nav_path` uses `NavigationServer3D::map_get_path` which returns a `PackedVector3Array`. We compute the path length by summing segment distances.
- The navigation map is retrieved via `get_maps()[0]` — Godot typically has a single default map. Multiple-map support is out of scope for M3.
- `resolve_node_public` exposes the existing private `resolve_node` for use by `action_handler.rs`.

**Acceptance Criteria:**
- [ ] `raycast(from, to)` returns clear/blocked with hit info
- [ ] Raycast respects collision_mask parameter
- [ ] `get_nav_path(from, to)` returns nav distance and path info
- [ ] `get_nav_path` returns meaningful error when no nav map exists
- [ ] `resolve_node_position` returns position and forward vector for Node3D and Node2D

---

### Unit 8: Query Handler Updates (`stage-godot`)

**File:** `crates/stage-godot/src/query_handler.rs` (modify existing)

Add dispatch for the new `execute_action`, `spatial_query`, and `resolve_node` methods.

```rust
// Add to imports:
use stage_protocol::query::{
    ActionRequest, ActionResponse,
    SpatialQueryRequest, RaycastResponse, NavPathResponse, ResolveNodeResponse,
};
use crate::action_handler;

// Add to handle_query match:
pub fn handle_query(
    id: String,
    method: &str,
    params: serde_json::Value,
    collector: &StageCollector,
) -> Message {
    let result = match method {
        "get_snapshot_data" => handle_get_snapshot_data(params, collector),
        "get_frame_info" => handle_get_frame_info(collector),
        "get_node_inspect" => handle_get_node_inspect(params, collector),
        "get_scene_tree" => handle_get_scene_tree(params, collector),
        "execute_action" => handle_execute_action(params, collector),
        "spatial_query" => handle_spatial_query(params, collector),
        _ => Err(QueryError {
            code: "method_not_found".to_string(),
            message: format!("Unknown query method: {method}"),
        }),
    };
    // ... existing result matching ...
}

fn handle_execute_action(
    params: serde_json::Value,
    collector: &StageCollector,
) -> Result<serde_json::Value, QueryError> {
    let request: ActionRequest = parse_params(params)?;
    let response = action_handler::execute_action(&request, collector)
        .map_err(|e| QueryError {
            code: "action_failed".to_string(),
            message: e,
        })?;
    to_json_value(&response)
}

fn handle_spatial_query(
    params: serde_json::Value,
    collector: &StageCollector,
) -> Result<serde_json::Value, QueryError> {
    let request: SpatialQueryRequest = parse_params(params)?;
    match request {
        SpatialQueryRequest::Raycast { from, to, collision_mask } => {
            let from_pos = resolve_query_origin(&from, collector)?;
            let to_pos = resolve_query_origin(&to, collector)?;
            let from_v3 = Vector3::new(from_pos[0] as f32, from_pos[1] as f32, from_pos[2] as f32);
            let to_v3 = Vector3::new(to_pos[0] as f32, to_pos[1] as f32, to_pos[2] as f32);
            let result = collector.raycast(from_v3, to_v3, collision_mask)
                .map_err(|e| QueryError { code: "query_failed".into(), message: e })?;
            to_json_value(&result)
        }
        SpatialQueryRequest::PathDistance { from, to } => {
            let from_pos = resolve_query_origin(&from, collector)?;
            let to_pos = resolve_query_origin(&to, collector)?;
            let from_v3 = Vector3::new(from_pos[0] as f32, from_pos[1] as f32, from_pos[2] as f32);
            let to_v3 = Vector3::new(to_pos[0] as f32, to_pos[1] as f32, to_pos[2] as f32);
            let result = collector.get_nav_path(from_v3, to_v3)
                .map_err(|e| QueryError { code: "query_failed".into(), message: e })?;
            to_json_value(&result)
        }
        SpatialQueryRequest::ResolveNode { path } => {
            let result = collector.resolve_node_position(&path)
                .map_err(|e| QueryError { code: "node_not_found".into(), message: e })?;
            to_json_value(&result)
        }
    }
}

/// Resolve a QueryOrigin to a position array.
fn resolve_query_origin(
    origin: &QueryOrigin,
    collector: &StageCollector,
) -> Result<Vec<f64>, QueryError> {
    match origin {
        QueryOrigin::Position(pos) => Ok(pos.clone()),
        QueryOrigin::Node(path) => {
            let resolved = collector.resolve_node_position(path)
                .map_err(|e| QueryError { code: "node_not_found".into(), message: e })?;
            Ok(resolved.position)
        }
    }
}
```

**Implementation Notes:**
- `handle_execute_action` and `handle_spatial_query` follow the same parse → execute → serialize pattern as existing handlers.
- `resolve_query_origin` handles the dual node-path/position origin used by raycast and path_distance queries.
- The `advance_frames` action needs special handling (deferred response). When the action handler detects an advance request, it should return a special marker that `handle_query` uses to signal the TCP server. This is the one case where the response is not sent immediately. See Unit 6 for the mechanism.

**Acceptance Criteria:**
- [ ] `execute_action` dispatches to action_handler for all action types
- [ ] `spatial_query` dispatches raycast to collector.raycast()
- [ ] `spatial_query` dispatches path_distance to collector.get_nav_path()
- [ ] `resolve_node` dispatches to collector.resolve_node_position()
- [ ] `resolve_query_origin` handles both node paths and position arrays
- [ ] Unknown methods still return `method_not_found`

---

### Unit 9: MCP `spatial_action` Tool (`stage-server`)

**File:** `crates/stage-server/src/mcp/action.rs` (new file)

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// MCP parameters for the spatial_action tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialActionParams {
    /// Action to perform.
    #[schemars(description = "Action type: pause, advance_frames, advance_time, teleport, set_property, call_method, emit_signal, spawn_node, remove_node")]
    pub action: String,

    /// Target node path (required for teleport, set_property, call_method, emit_signal, remove_node).
    #[schemars(description = "Node path for the action target")]
    pub node: Option<String>,

    /// For pause: whether to pause (true) or unpause (false).
    pub paused: Option<bool>,

    /// For advance_frames: number of physics frames to advance.
    pub frames: Option<u32>,

    /// For advance_time: seconds to advance.
    pub seconds: Option<f64>,

    /// For teleport: target position [x, y, z] or [x, y].
    pub position: Option<Vec<f64>>,

    /// For teleport: target rotation in degrees (yaw for 3D, angle for 2D).
    pub rotation_deg: Option<f64>,

    /// For set_property: property name.
    pub property: Option<String>,

    /// For set_property: new value.
    pub value: Option<serde_json::Value>,

    /// For emit_signal: signal name.
    pub signal: Option<String>,

    /// For emit_signal/call_method: arguments.
    pub args: Option<Vec<serde_json::Value>>,

    /// For call_method: method name.
    pub method: Option<String>,

    /// For call_method: method arguments (alias for args).
    pub method_args: Option<Vec<serde_json::Value>>,

    /// For spawn_node: scene resource path.
    pub scene_path: Option<String>,

    /// For spawn_node: parent node path.
    pub parent: Option<String>,

    /// For spawn_node: name for the new node.
    pub name: Option<String>,

    /// Whether to return a spatial_delta after the action.
    #[serde(default)]
    pub return_delta: bool,
}

/// Build the addon ActionRequest from MCP params.
/// Validates required fields per action type.
pub fn build_action_request(
    params: &SpatialActionParams,
) -> Result<stage_protocol::query::ActionRequest, rmcp::model::ErrorData> {
    use stage_protocol::query::ActionRequest;
    use rmcp::model::ErrorData as McpError;

    match params.action.as_str() {
        "pause" => {
            let paused = params.paused.ok_or_else(|| {
                McpError::invalid_params("'paused' (bool) is required for pause action", None)
            })?;
            Ok(ActionRequest::Pause { paused })
        }
        "advance_frames" => {
            let frames = params.frames.ok_or_else(|| {
                McpError::invalid_params("'frames' (int) is required for advance_frames action", None)
            })?;
            Ok(ActionRequest::AdvanceFrames { frames })
        }
        "advance_time" => {
            let seconds = params.seconds.ok_or_else(|| {
                McpError::invalid_params("'seconds' (float) is required for advance_time action", None)
            })?;
            Ok(ActionRequest::AdvanceTime { seconds })
        }
        "teleport" => {
            let node = params.node.as_ref().ok_or_else(|| {
                McpError::invalid_params("'node' is required for teleport action", None)
            })?;
            let position = params.position.as_ref().ok_or_else(|| {
                McpError::invalid_params("'position' ([x,y,z] or [x,y]) is required for teleport action", None)
            })?;
            Ok(ActionRequest::Teleport {
                path: node.clone(),
                position: position.clone(),
                rotation_deg: params.rotation_deg,
            })
        }
        "set_property" => {
            let node = params.node.as_ref().ok_or_else(|| {
                McpError::invalid_params("'node' is required for set_property action", None)
            })?;
            let property = params.property.as_ref().ok_or_else(|| {
                McpError::invalid_params("'property' is required for set_property action", None)
            })?;
            let value = params.value.as_ref().ok_or_else(|| {
                McpError::invalid_params("'value' is required for set_property action", None)
            })?;
            Ok(ActionRequest::SetProperty {
                path: node.clone(),
                property: property.clone(),
                value: value.clone(),
            })
        }
        "call_method" => {
            let node = params.node.as_ref().ok_or_else(|| {
                McpError::invalid_params("'node' is required for call_method action", None)
            })?;
            let method = params.method.as_ref().ok_or_else(|| {
                McpError::invalid_params("'method' is required for call_method action", None)
            })?;
            let args = params.method_args.as_ref()
                .or(params.args.as_ref())
                .cloned()
                .unwrap_or_default();
            Ok(ActionRequest::CallMethod {
                path: node.clone(),
                method: method.clone(),
                args,
            })
        }
        "emit_signal" => {
            let node = params.node.as_ref().ok_or_else(|| {
                McpError::invalid_params("'node' is required for emit_signal action", None)
            })?;
            let signal = params.signal.as_ref().ok_or_else(|| {
                McpError::invalid_params("'signal' is required for emit_signal action", None)
            })?;
            let args = params.args.as_ref().cloned().unwrap_or_default();
            Ok(ActionRequest::EmitSignal {
                path: node.clone(),
                signal: signal.clone(),
                args,
            })
        }
        "spawn_node" => {
            let scene_path = params.scene_path.as_ref().ok_or_else(|| {
                McpError::invalid_params("'scene_path' is required for spawn_node action", None)
            })?;
            let parent = params.parent.as_ref().ok_or_else(|| {
                McpError::invalid_params("'parent' is required for spawn_node action", None)
            })?;
            Ok(ActionRequest::SpawnNode {
                scene_path: scene_path.clone(),
                parent: parent.clone(),
                name: params.name.clone(),
                position: params.position.clone(),
            })
        }
        "remove_node" => {
            let node = params.node.as_ref().ok_or_else(|| {
                McpError::invalid_params("'node' is required for remove_node action", None)
            })?;
            Ok(ActionRequest::RemoveNode { path: node.clone() })
        }
        other => Err(McpError::invalid_params(
            format!("Unknown action type: '{other}'. Valid actions: pause, advance_frames, advance_time, teleport, set_property, call_method, emit_signal, spawn_node, remove_node"),
            None,
        )),
    }
}
```

**File:** `crates/stage-server/src/mcp/mod.rs` (add tool to router)

```rust
pub mod action;

use action::{SpatialActionParams, build_action_request};

// Add to #[tool_router] impl:

    /// Manipulate game state for debugging. Actions: pause/unpause, advance
    /// frames or time, teleport nodes, set properties, call methods, emit
    /// signals, spawn or remove nodes.
    #[tool(description = "Manipulate game state for debugging. Actions: pause (pause/unpause scene), advance_frames (step N physics frames while paused), advance_time (step N seconds while paused), teleport (move node to position), set_property (change a property), call_method (call a method), emit_signal (emit a signal), spawn_node (instantiate a scene), remove_node (queue_free a node). Use return_delta=true to get a spatial delta after the action.")]
    pub async fn spatial_action(
        &self,
        Parameters(params): Parameters<SpatialActionParams>,
    ) -> Result<String, McpError> {
        let action_request = build_action_request(&params)?;
        let data = query_addon(
            &self.state,
            "execute_action",
            serialize_params(&action_request)?,
        ).await?;

        let mut response: serde_json::Value = data;

        // Inject budget
        let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
        let used = stage_core::budget::estimate_tokens(json_bytes);
        inject_budget(&mut response, used, 500);

        // return_delta is deferred to M4 (requires delta engine)
        // For now, include a note if requested
        if params.return_delta {
            if let serde_json::Value::Object(ref mut map) = response {
                map.insert("delta".into(), serde_json::json!(null));
                map.insert("delta_note".into(), serde_json::json!(
                    "return_delta requires the delta engine (M4). Use spatial_snapshot after the action for now."
                ));
            }
        }

        serialize_response(&response)
    }
```

**Implementation Notes:**
- `build_action_request` validates all required fields per action type and returns clear error messages telling the agent what's missing.
- `return_delta` is acknowledged but deferred to M4. The response includes a note suggesting `spatial_snapshot` as a workaround. This matches the roadmap: M3 establishes the action response shape, M4 wires in delta.
- `method_args` and `args` are both accepted for `call_method` (CONTRACT.md uses `method_args`, but `args` is more intuitive).

**Acceptance Criteria:**
- [ ] `spatial_action(action: "pause", paused: true)` pauses the game
- [ ] `spatial_action(action: "teleport", node: "...", position: [...])` teleports node
- [ ] Missing required params return clear `invalid_params` errors
- [ ] Unknown action types return `invalid_params` with valid action list
- [ ] `return_delta: true` includes a null delta with a note (M4 placeholder)
- [ ] Budget block is included on every response

---

### Unit 10: MCP `spatial_query` Tool (`stage-server`)

**File:** `crates/stage-server/src/mcp/query.rs` (new file)

```rust
use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use stage_core::{
    bearing::{self, perspective_from_forward, relative_position},
    budget::{estimate_tokens, resolve_budget, SnapshotBudgetDefaults},
    index::NearestResult,
    types::{vec_to_array3, Position3},
};
use stage_protocol::query::{
    NavPathResponse, QueryOrigin, RaycastResponse, ResolveNodeResponse, SpatialQueryRequest,
};

use crate::tcp::query_addon;
use super::{inject_budget, serialize_params, serialize_response, deserialize_response};

/// MCP parameters for the spatial_query tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialQueryParams {
    /// Type of spatial query.
    #[schemars(description = "Query type: nearest, radius, raycast, area, path_distance, relationship")]
    pub query_type: String,

    /// Origin — node path or world position [x, y, z].
    #[schemars(description = "Origin: node path (string) or position array [x,y,z]")]
    pub from: serde_json::Value,

    /// Target — for raycast and relationship queries.
    #[schemars(description = "Target: node path (string) or position array [x,y,z]")]
    pub to: Option<serde_json::Value>,

    /// For nearest queries: number of results.
    #[serde(default = "default_k")]
    pub k: usize,

    /// For radius/area queries: search radius.
    #[serde(default = "default_query_radius")]
    pub radius: f64,

    /// Filter by group membership.
    pub groups: Option<Vec<String>>,

    /// Filter by class.
    pub class_filter: Option<Vec<String>>,

    /// Token budget for the response.
    pub token_budget: Option<u32>,
}

fn default_k() -> usize { 5 }
fn default_query_radius() -> f64 { 20.0 }

/// Parse a "from" or "to" value into a QueryOrigin.
fn parse_origin(value: &serde_json::Value) -> Result<QueryOrigin, McpError> {
    match value {
        serde_json::Value::String(s) => Ok(QueryOrigin::Node(s.clone())),
        serde_json::Value::Array(arr) => {
            let coords: Result<Vec<f64>, _> = arr.iter().map(|v| {
                v.as_f64().ok_or_else(|| McpError::invalid_params("Position array must contain numbers", None))
            }).collect();
            Ok(QueryOrigin::Position(coords?))
        }
        _ => Err(McpError::invalid_params(
            "Origin must be a node path (string) or position array [x,y,z]",
            None,
        )),
    }
}

/// Resolve a query origin to a Position3 and optional forward vector.
/// For node origins, queries the addon. For positions, returns directly.
async fn resolve_origin(
    origin: &QueryOrigin,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<(Position3, Option<[f64; 3]>), McpError> {
    match origin {
        QueryOrigin::Position(pos) => {
            Ok((vec_to_array3(pos), None))
        }
        QueryOrigin::Node(path) => {
            let req = SpatialQueryRequest::ResolveNode { path: path.clone() };
            let data = query_addon(state, "spatial_query", serialize_params(&req)?).await?;
            let resolved: ResolveNodeResponse = deserialize_response(data)?;
            Ok((vec_to_array3(&resolved.position), Some(vec_to_array3(&resolved.forward))))
        }
    }
}

/// Build the nearest query response from the spatial index.
fn build_nearest_response(
    results: &[NearestResult],
    from_pos: Position3,
    from_forward: Option<[f64; 3]>,
) -> serde_json::Value {
    let perspective = from_forward
        .map(|fwd| perspective_from_forward(from_pos, fwd))
        .unwrap_or_else(|| bearing::perspective_from_yaw(from_pos, 0.0));

    let entries: Vec<serde_json::Value> = results.iter().map(|r| {
        let rel = relative_position(&perspective, r.position, false);
        serde_json::json!({
            "path": r.path,
            "dist": (r.distance * 10.0).round() / 10.0,
            "bearing": rel.bearing,
            "class": r.class,
        })
    }).collect();

    serde_json::json!({
        "query": "nearest",
        "results": entries,
    })
}

/// Build the radius query response from the spatial index.
fn build_radius_response(
    results: &[NearestResult],
    from_pos: Position3,
    from_forward: Option<[f64; 3]>,
    radius: f64,
) -> serde_json::Value {
    let perspective = from_forward
        .map(|fwd| perspective_from_forward(from_pos, fwd))
        .unwrap_or_else(|| bearing::perspective_from_yaw(from_pos, 0.0));

    let entries: Vec<serde_json::Value> = results.iter().map(|r| {
        let rel = relative_position(&perspective, r.position, false);
        serde_json::json!({
            "path": r.path,
            "dist": (r.distance * 10.0).round() / 10.0,
            "bearing": rel.bearing,
            "class": r.class,
        })
    }).collect();

    serde_json::json!({
        "query": "radius",
        "radius": radius,
        "results": entries,
    })
}

/// Build the relationship query response by composing multiple queries.
async fn build_relationship_response(
    from_origin: &QueryOrigin,
    to_origin: &QueryOrigin,
    from_pos: Position3,
    from_forward: Option<[f64; 3]>,
    to_pos: Position3,
    to_forward: Option<[f64; 3]>,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<serde_json::Value, McpError> {
    let distance = bearing::distance(from_pos, to_pos);

    // Bearing from A to B
    let persp_a = from_forward
        .map(|fwd| perspective_from_forward(from_pos, fwd))
        .unwrap_or_else(|| bearing::perspective_from_yaw(from_pos, 0.0));
    let rel_a_to_b = relative_position(&persp_a, to_pos, false);

    // Bearing from B to A
    let persp_b = to_forward
        .map(|fwd| perspective_from_forward(to_pos, fwd))
        .unwrap_or_else(|| bearing::perspective_from_yaw(to_pos, 0.0));
    let rel_b_to_a = relative_position(&persp_b, from_pos, false);

    // Raycast for line of sight
    let raycast_req = SpatialQueryRequest::Raycast {
        from: from_origin.clone(),
        to: to_origin.clone(),
        collision_mask: None,
    };
    let raycast_data = query_addon(state, "spatial_query", serialize_params(&raycast_req)?).await?;
    let raycast: RaycastResponse = deserialize_response(raycast_data)?;

    // Optional nav distance
    let nav_distance = {
        let nav_req = SpatialQueryRequest::PathDistance {
            from: from_origin.clone(),
            to: to_origin.clone(),
        };
        match query_addon(state, "spatial_query", serialize_params(&nav_req)?).await {
            Ok(data) => {
                let nav: NavPathResponse = deserialize_response(data)?;
                if nav.traversable { Some(nav.nav_distance) } else { None }
            }
            Err(_) => None,
        }
    };

    let mut result = serde_json::json!({
        "distance": (distance * 10.0).round() / 10.0,
        "bearing_from_a": rel_a_to_b.bearing,
        "bearing_from_b": rel_b_to_a.bearing,
        "line_of_sight": raycast.clear,
    });

    if let Some(elev) = &rel_a_to_b.elevation {
        result["elevation_diff"] = match elev {
            stage_core::types::Elevation::Level => serde_json::json!(0.0),
            stage_core::types::Elevation::Above(d) => serde_json::json!(d),
            stage_core::types::Elevation::Below(d) => serde_json::json!(-d),
        };
    }
    if !raycast.clear {
        if let Some(ref occ) = raycast.blocked_by {
            result["occluder"] = serde_json::json!(occ);
        }
    }
    if let Some(nav) = nav_distance {
        result["nav_distance"] = serde_json::json!((nav * 10.0).round() / 10.0);
    }

    Ok(serde_json::json!({
        "query": "relationship",
        "from": from_origin,
        "to": to_origin,
        "result": result,
    }))
}
```

**File:** `crates/stage-server/src/mcp/mod.rs` (add tool to router)

```rust
pub mod query;

use query::{SpatialQueryParams, parse_origin, resolve_origin, build_nearest_response, build_radius_response, build_relationship_response};

// Add to #[tool_router] impl:

    /// Targeted spatial questions: nearest nodes, radius search, raycast line-of-sight,
    /// navigation path distance, or mutual relationship between two nodes.
    #[tool(description = "Targeted spatial questions. Query types: 'nearest' (K nearest nodes to a point/node), 'radius' (all nodes within radius), 'raycast' (line-of-sight check between two points/nodes), 'path_distance' (navmesh distance), 'relationship' (mutual spatial relationship between two nodes), 'area' (alias for radius).")]
    pub async fn spatial_query(
        &self,
        Parameters(params): Parameters<SpatialQueryParams>,
    ) -> Result<String, McpError> {
        let from_origin = parse_origin(&params.from)?;
        let groups = params.groups.as_deref().unwrap_or(&[]);
        let class_filter = params.class_filter.as_deref().unwrap_or(&[]);
        let budget_limit = resolve_budget(params.token_budget, 500, SnapshotBudgetDefaults::HARD_CAP);

        let mut response = match params.query_type.as_str() {
            "nearest" => {
                let (from_pos, from_fwd) = resolve_origin(&from_origin, &self.state).await?;
                let state = self.state.lock().await;
                let results = state.spatial_index.nearest(from_pos, params.k, groups, class_filter);
                drop(state);
                build_nearest_response(&results, from_pos, from_fwd)
            }
            "radius" | "area" => {
                let (from_pos, from_fwd) = resolve_origin(&from_origin, &self.state).await?;
                let state = self.state.lock().await;
                let results = state.spatial_index.within_radius(from_pos, params.radius, groups, class_filter);
                drop(state);
                build_radius_response(&results, from_pos, from_fwd, params.radius)
            }
            "raycast" => {
                let to_val = params.to.as_ref().ok_or_else(|| {
                    McpError::invalid_params("'to' is required for raycast query", None)
                })?;
                let to_origin = parse_origin(to_val)?;
                let req = SpatialQueryRequest::Raycast {
                    from: from_origin.clone(),
                    to: to_origin.clone(),
                    collision_mask: None,
                };
                let data = query_addon(&self.state, "spatial_query", serialize_params(&req)?).await?;
                let raycast: RaycastResponse = deserialize_response(data)?;
                serde_json::json!({
                    "query": "raycast",
                    "from": params.from,
                    "to": to_val,
                    "result": raycast,
                })
            }
            "path_distance" => {
                let to_val = params.to.as_ref().ok_or_else(|| {
                    McpError::invalid_params("'to' is required for path_distance query", None)
                })?;
                let to_origin = parse_origin(to_val)?;
                let req = SpatialQueryRequest::PathDistance {
                    from: from_origin,
                    to: to_origin,
                };
                let data = query_addon(&self.state, "spatial_query", serialize_params(&req)?).await?;
                let nav: NavPathResponse = deserialize_response(data)?;
                serde_json::json!({
                    "query": "path_distance",
                    "from": params.from,
                    "to": to_val,
                    "result": nav,
                })
            }
            "relationship" => {
                let to_val = params.to.as_ref().ok_or_else(|| {
                    McpError::invalid_params("'to' is required for relationship query", None)
                })?;
                let to_origin = parse_origin(to_val)?;
                let (from_pos, from_fwd) = resolve_origin(&from_origin, &self.state).await?;
                let (to_pos, to_fwd) = resolve_origin(&to_origin, &self.state).await?;
                build_relationship_response(
                    &from_origin, &to_origin,
                    from_pos, from_fwd, to_pos, to_fwd,
                    &self.state,
                ).await?
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("Unknown query_type: '{other}'. Valid types: nearest, radius, raycast, path_distance, relationship, area"),
                    None,
                ));
            }
        };

        // Add "from" field to response
        if let serde_json::Value::Object(ref mut map) = response {
            if !map.contains_key("from") {
                map.insert("from".into(), params.from.clone());
            }
        }

        // Inject budget
        let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
        let used = estimate_tokens(json_bytes);
        inject_budget(&mut response, used, budget_limit);

        serialize_response(&response)
    }
```

**Implementation Notes:**
- `nearest` and `radius`/`area` queries run entirely server-side using the spatial index. No addon TCP call is needed (except for resolving node paths to positions).
- `raycast` and `path_distance` delegate to the addon because they need Godot's physics/navigation servers.
- `relationship` is a composite query: it resolves both origins, computes bearings server-side, and performs a raycast + optional nav path query via the addon.
- `area` is an alias for `radius` (same behavior — nodes within a sphere).
- The spatial index may be empty if no snapshot has been taken yet. In that case, `nearest`/`radius` return empty results. The agent should take a snapshot first.

**Acceptance Criteria:**
- [ ] `spatial_query(query_type: "nearest", from: "player", k: 5)` returns 5 nearest nodes
- [ ] `spatial_query(query_type: "radius", from: "player", radius: 15)` returns nodes within 15 units
- [ ] `spatial_query(query_type: "raycast", from: "enemy", to: "player")` returns line-of-sight result
- [ ] `spatial_query(query_type: "path_distance", from: "enemy", to: "player")` returns nav distance
- [ ] `spatial_query(query_type: "relationship", from: "enemy", to: "player")` returns mutual spatial relationship
- [ ] Group and class filters work on nearest/radius queries
- [ ] Missing required params return clear error messages
- [ ] Budget block is included on every response

---

### Unit 11: Error Handling Updates

**File:** `crates/stage-protocol/src/query.rs` (verify error codes)

No new types needed — errors use existing `Message::Error { code, message }`. New error codes used by M3:

| Code | Context | Source |
|---|---|---|
| `action_failed` | Action could not be executed (e.g., node not found, bad property) | action_handler |
| `query_failed` | Spatial query failed (e.g., no physics world, no nav map) | collector |
| `method_not_found` | call_method target doesn't exist on node | action_handler |
| `not_paused` | advance_frames/advance_time called while not paused | action_handler |
| `node_not_found` | Node path doesn't exist (reused from M1/M2) | collector |
| `dimension_mismatch` | 3D operation on 2D node or vice versa | action_handler |

Error messages should be actionable — include the node path, method name, or property name that failed.

**Acceptance Criteria:**
- [ ] All new errors include `code` and `message` fields
- [ ] Error messages include the relevant path/property/method that failed
- [ ] `not_paused` error suggests using `pause` action first

---

### Unit 12: `stage-core` lib.rs Exports and Cargo.toml Updates

**File:** `crates/stage-core/src/lib.rs` (add module)

```rust
pub mod bearing;
pub mod budget;
pub mod cluster;
pub mod index;  // NEW
pub mod types;
```

**File:** `crates/stage-core/Cargo.toml` (add rstar dependency)

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
rstar = "0.12"
```

**File:** `crates/stage-godot/src/lib.rs` (add module)

```rust
mod collector;
mod query_handler;
mod tcp_server;
mod action_handler;  // NEW
```

**File:** `crates/stage-server/src/mcp/mod.rs` (add modules)

```rust
pub mod action;      // NEW
pub mod inspect;
pub mod query;       // NEW
pub mod scene_tree;
pub mod snapshot;
```

**Acceptance Criteria:**
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo clippy --workspace` passes
- [ ] `cargo test --workspace` passes

---

## Implementation Order

1. **Unit 1: Protocol Action Types** — No dependencies, foundation for everything else
2. **Unit 2: Protocol Query Types** — No dependencies, foundation for spatial queries
3. **Unit 3: Spatial Index** — Depends on rstar in Cargo.toml, pure logic
4. **Unit 12: Module exports and Cargo.toml** — Wire up new modules
5. **Unit 5: Action Handler** — Depends on Units 1, 12. Core addon logic.
6. **Unit 7: Spatial Query Handler** — Depends on Unit 2. Raycast + nav path in addon.
7. **Unit 8: Query Handler Updates** — Depends on Units 5, 7. Wire dispatch.
8. **Unit 6: Frame Advance** — Depends on Unit 5. TCP server cooperation.
9. **Unit 4: Index Integration** — Depends on Unit 3. Wire into session state.
10. **Unit 9: MCP spatial_action** — Depends on Units 5, 8, 9. Server-side tool.
11. **Unit 10: MCP spatial_query** — Depends on Units 3, 4, 7, 8. Server-side tool.
12. **Unit 11: Error Handling** — Verify across all units.

**Critical path:** Units 1 → 5 → 8 → 9 (action tool end-to-end) and Units 2 → 7 → 8 → 10 (query tool end-to-end) can proceed in parallel after Units 1-2.

---

## Testing

### Unit Tests: `crates/stage-protocol/src/query.rs`

```rust
#[test]
fn action_request_tagged_enum_serde() {
    let req = ActionRequest::Teleport {
        path: "enemy".into(),
        position: vec![5.0, 0.0, -3.0],
        rotation_deg: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains(r#""action":"teleport""#));
    let parsed: ActionRequest = serde_json::from_str(&json).unwrap();
    // verify round-trip
}

#[test]
fn query_origin_untagged_serde() {
    let node: QueryOrigin = serde_json::from_str(r#""player""#).unwrap();
    assert!(matches!(node, QueryOrigin::Node(s) if s == "player"));

    let pos: QueryOrigin = serde_json::from_str(r#"[1.0, 2.0, 3.0]"#).unwrap();
    assert!(matches!(pos, QueryOrigin::Position(v) if v.len() == 3));
}

#[test]
fn action_response_round_trip() {
    let resp = ActionResponse {
        action: "teleport".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("previous_position".into(), serde_json::json!([1.0, 2.0, 3.0])),
        ]),
        frame: 100,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: ActionResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.action, "teleport");
}
```

### Unit Tests: `crates/stage-core/src/index.rs`

```rust
#[test]
fn nearest_returns_k_closest() {
    let entities = vec![
        IndexedEntity { path: "a".into(), class: "Node3D".into(), position: [0.0, 0.0, 0.0], groups: vec![] },
        IndexedEntity { path: "b".into(), class: "Node3D".into(), position: [5.0, 0.0, 0.0], groups: vec![] },
        IndexedEntity { path: "c".into(), class: "Node3D".into(), position: [10.0, 0.0, 0.0], groups: vec![] },
    ];
    let index = SpatialIndex::build(entities);
    let results = index.nearest([0.0, 0.0, 0.0], 2, &[], &[]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].path, "a");
    assert_eq!(results[1].path, "b");
}

#[test]
fn within_radius_filters_by_distance() {
    let entities = vec![
        IndexedEntity { path: "close".into(), class: "Node3D".into(), position: [3.0, 0.0, 0.0], groups: vec![] },
        IndexedEntity { path: "far".into(), class: "Node3D".into(), position: [100.0, 0.0, 0.0], groups: vec![] },
    ];
    let index = SpatialIndex::build(entities);
    let results = index.within_radius([0.0, 0.0, 0.0], 10.0, &[], &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "close");
}

#[test]
fn group_filter_applies() {
    let entities = vec![
        IndexedEntity { path: "enemy".into(), class: "Node3D".into(), position: [1.0, 0.0, 0.0], groups: vec!["enemies".into()] },
        IndexedEntity { path: "pickup".into(), class: "Area3D".into(), position: [2.0, 0.0, 0.0], groups: vec!["pickups".into()] },
    ];
    let index = SpatialIndex::build(entities);
    let results = index.nearest([0.0, 0.0, 0.0], 5, &["enemies".to_string()], &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "enemy");
}

#[test]
fn empty_index_returns_empty() {
    let index = SpatialIndex::empty();
    let results = index.nearest([0.0, 0.0, 0.0], 5, &[], &[]);
    assert!(results.is_empty());
}
```

### Unit Tests: `crates/stage-server/src/mcp/action.rs`

```rust
#[test]
fn build_action_request_teleport() {
    let params = SpatialActionParams {
        action: "teleport".into(),
        node: Some("enemy".into()),
        position: Some(vec![5.0, 0.0, -3.0]),
        rotation_deg: Some(90.0),
        // ... other fields None/default ...
    };
    let req = build_action_request(&params).unwrap();
    assert!(matches!(req, ActionRequest::Teleport { .. }));
}

#[test]
fn build_action_request_missing_node() {
    let params = SpatialActionParams {
        action: "teleport".into(),
        node: None,  // missing!
        position: Some(vec![5.0, 0.0, -3.0]),
        // ...
    };
    assert!(build_action_request(&params).is_err());
}

#[test]
fn build_action_request_unknown_action() {
    let params = SpatialActionParams {
        action: "fly".into(),
        // ...
    };
    assert!(build_action_request(&params).is_err());
}
```

### Unit Tests: `crates/stage-server/src/mcp/query.rs`

```rust
#[test]
fn parse_origin_string() {
    let val = serde_json::json!("player");
    let origin = parse_origin(&val).unwrap();
    assert!(matches!(origin, QueryOrigin::Node(s) if s == "player"));
}

#[test]
fn parse_origin_array() {
    let val = serde_json::json!([1.0, 2.0, 3.0]);
    let origin = parse_origin(&val).unwrap();
    assert!(matches!(origin, QueryOrigin::Position(v) if v.len() == 3));
}

#[test]
fn parse_origin_invalid() {
    let val = serde_json::json!(42);
    assert!(parse_origin(&val).is_err());
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

# Verify new protocol types serialize correctly
cargo test -p stage-protocol -- action query_origin

# Verify spatial index
cargo test -p stage-core -- index

# Verify action builder
cargo test -p stage-server -- action

# Verify query origin parsing
cargo test -p stage-server -- query

# Manual integration test (requires running Godot):
# 1. Start Godot with stage addon
# 2. Start stage-server
# 3. Call spatial_action(action: "pause", paused: true) — game freezes
# 4. Call spatial_action(action: "teleport", node: "some_node", position: [0,0,0])
# 5. Call spatial_query(query_type: "nearest", from: "player", k: 3)
# 6. Call spatial_query(query_type: "raycast", from: "player", to: "enemy")
```
