# Design: Milestone 9 — 2D Support

## Overview

M9 makes Stage work correctly with 2D Godot games. All tools produce 2D-appropriate output: `[x, y]` positions, single-angle rotation, no elevation, Camera2D viewport culling, 2D physics queries, and a grid-hash spatial index. The MCP tool interfaces remain identical — differences are in the *values* within response structures.

**Depends on:** M1-M8 (layered on top of working 3D implementation)

**Exit Criteria:** Agent calls `spatial_snapshot` in a 2D platformer → gets `[x, y]` positions, correct bearings (no elevation), Camera2D viewport culling. All 9 tools work correctly in 2D. Raycasts use 2D physics. Recordings capture 2D state correctly.

---

## Architecture Decision: Dimension as a Thread-Through Enum

**Decision:** Introduce a `SceneDimensions` enum (`Two | Three | Mixed`) that flows from handshake through `SessionState` and is consulted at each layer boundary where 3D assumptions currently exist.

**Rationale:**
- The existing code already has `scene_dimensions: u32` in the handshake and `HandshakeInfo` — we promote this to a typed enum
- Each layer has a small number of dimension-dependent code paths (collector: ~5 methods, core: bearing + index, server: snapshot output formatting)
- The protocol layer (`Vec<f64>` for positions) already handles variable-length coordinate arrays — no wire format changes needed
- Mixed scenes collect both Node2D and Node3D; the server treats each entity by its own dimension

**Consequence:** Every function that currently assumes 3D (`[f64; 3]`, elevation, XZ-plane projection, R-tree) gains a dimension-aware variant or conditional branch. The core types gain `Position2 = [f64; 2]` and a `Position` enum.

---

## Architecture Decision: Dual Spatial Index

**Decision:** `SpatialIndex` becomes an enum wrapping either an R-tree (3D) or a grid hash (2D). The public API (`nearest`, `within_radius`) remains identical. The server selects the index type based on `scene_dimensions` from the handshake.

**Rationale:**
- SPEC.md specifies R-tree for 3D and grid hash for 2D
- A grid hash is simpler and faster for uniform 2D distributions
- The spatial index is rebuilt on every snapshot — switching types on scene change is trivial
- The `NearestResult` struct already uses `Position3`; we'll keep it as-is and zero the Z component for 2D entities (simpler than making it generic)

---

## Current State Analysis

### Already 2D-aware (no changes needed):
1. **Protocol `EntityData`** — uses `Vec<f64>` for position/rotation/velocity (handles any length)
2. **Protocol `PerspectiveData`** — uses `Vec<f64>` (handles any length)
3. **Action handler `teleport`** — already handles both Node3D and Node2D (action_handler.rs:135-207)
4. **Action handler `spawn_node`** — already handles both Node3D and Node2D (action_handler.rs:341-354)
5. **`json_to_variant`** — detects Vector2 vs Vector3 from array length (action_handler.rs:406-418)
6. **`static_classes.rs`** — already includes `StaticBody2D`
7. **Recording `FrameEntityData`** — uses `Vec<f64>` for position/rotation/velocity

### Must be changed for 2D:
1. **`detect_scene_dimensions()`** — hardcoded to return `3` (tcp_server.rs:390-392)
2. **`collect_entities_recursive()`** — only tries `Node3D` cast (collector.rs:163)
3. **`collect_single_entity()`** — calls Node3D methods (collector.rs:203-243)
4. **`get_velocity()`** — only checks CharacterBody3D/RigidBody3D (collector.rs:246-254)
5. **`resolve_perspective()`** — only checks Camera3D/Node3D (collector.rs:104-150)
6. **`get_physics_data()`** — only checks CharacterBody3D (collector.rs:347-368)
7. **`get_transform_data()`** — uses Transform3D (collector.rs:371-385)
8. **`collect_spatial_context_raw()`** — only checks Node3D/Camera3D (collector.rs:702-758)
9. **`collect_nearby_recursive()`** — only checks Node3D (collector.rs:760-796)
10. **`collect_containing_areas()`** — only checks Area3D (collector.rs:799+)
11. **`collect_inspect_transform()`** — only checks Node3D (collector.rs:469-489)
12. **`collect_inspect_physics()`** — only checks CharacterBody3D/RigidBody3D (collector.rs:491-531)
13. **`collect_child_props()`** — only checks 3D classes (collector.rs:557-606)
14. **Core `bearing.rs`** — XZ plane projection, elevation always present
15. **Core `index.rs`** — R-tree with `[f64; 3]` only
16. **Core `types.rs`** — `Position3`, `Perspective` uses `[f64; 3]`
17. **Core `delta.rs`** — `EntitySnapshot.position: Position3`
18. **Server `snapshot.rs`** — `build_perspective()` assumes 3 elements, `OutputEntity.rot_y`
19. **Server `tcp.rs`** — `SessionState` doesn't store/use `scene_dimensions`

---

## Implementation Units

### Unit 1: SceneDimensions Enum in stage-protocol

**File:** `crates/stage-protocol/src/handshake.rs`

```rust
/// Scene coordinate system type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneDimensions {
    /// Pure 2D scene (Node2D root, Camera2D).
    #[serde(rename = "2")]
    Two,
    /// Pure 3D scene (Node3D root, Camera3D).
    #[serde(rename = "3")]
    Three,
    /// Mixed scene containing both Node2D and Node3D subtrees.
    #[serde(rename = "mixed")]
    Mixed,
}

impl SceneDimensions {
    pub fn is_2d(&self) -> bool { matches!(self, Self::Two) }
    pub fn is_3d(&self) -> bool { matches!(self, Self::Three) }
    pub fn is_mixed(&self) -> bool { matches!(self, Self::Mixed) }

    pub fn from_u32(v: u32) -> Self {
        match v {
            2 => Self::Two,
            3 => Self::Three,
            _ => Self::Mixed,
        }
    }
}
```

Update `Handshake` struct: change `scene_dimensions: u32` to `scene_dimensions: SceneDimensions`. Add `#[serde(deserialize_with = ...)]` or keep backward compat by accepting both u32 and string via untagged. Simplest approach: keep `u32` on the wire, add a helper method:

```rust
impl Handshake {
    pub fn dimensions(&self) -> SceneDimensions {
        SceneDimensions::from_u32(self.scene_dimensions)
    }
}
```

This avoids breaking the existing handshake wire format while providing typed access.

**File:** `crates/stage-protocol/src/static_classes.rs`

Add 2D static classes to `STATIC_CLASSES`:

```rust
pub const STATIC_CLASSES: &[&str] = &[
    // 3D
    "StaticBody3D",
    "CSGShape3D", "CSGBox3D", "CSGCylinder3D", "CSGMesh3D",
    "CSGPolygon3D", "CSGSphere3D", "CSGTorus3D", "CSGCombiner3D",
    "MeshInstance3D", "GridMap",
    "WorldEnvironment",
    "DirectionalLight3D", "OmniLight3D", "SpotLight3D",
    // 2D
    "StaticBody2D",
    "TileMapLayer",
    "Sprite2D",       // without script = static
    "PointLight2D", "DirectionalLight2D",
];
```

Update `classify_static_category`:

```rust
pub fn classify_static_category(class: &str) -> &'static str {
    match class {
        "StaticBody3D" | "StaticBody2D" => "collision",
        c if c.starts_with("CSG") => "csg",
        "GridMap" | "TileMapLayer" => "tilemap",
        "WorldEnvironment" => "environment",
        "MeshInstance3D" | "Sprite2D" => "visual",
        c if c.contains("Light") => "lights",
        _ => "other",
    }
}
```

**Implementation Notes:**
- The `SceneDimensions` enum lives in `handshake.rs` alongside the `Handshake` struct since it's intrinsically part of the connection protocol
- Re-export from `stage_protocol` lib.rs for convenience

**Acceptance Criteria:**
- [ ] `SceneDimensions` enum exists with `Two`, `Three`, `Mixed` variants
- [ ] `Handshake::dimensions()` returns the typed enum
- [ ] `SceneDimensions::from_u32()` maps 2→Two, 3→Three, other→Mixed
- [ ] 2D static classes added to `STATIC_CLASSES`
- [ ] `classify_static_category` handles 2D classes
- [ ] Existing handshake serde tests still pass (wire format unchanged)

---

### Unit 2: Scene Dimension Detection in stage-godot

**File:** `crates/stage-godot/src/tcp_server.rs`

Replace the hardcoded `detect_scene_dimensions`:

```rust
fn detect_scene_dimensions(&self) -> u32 {
    let Some(tree) = self.base().get_tree() else { return 3 };
    let Some(root) = tree.get_current_scene() else { return 3 };

    let has_2d = Self::has_node_type_recursive(&root, true);
    let has_3d = Self::has_node_type_recursive(&root, false);

    match (has_2d, has_3d) {
        (true, false) => 2,
        (false, true) => 3,
        (true, true) => 0,   // mixed
        (false, false) => 3,  // default to 3D if no spatial nodes
    }
}

/// Check if the scene tree contains Node2D (if `check_2d`) or Node3D nodes.
/// Stops at first match for efficiency.
fn has_node_type_recursive(node: &Gd<Node>, check_2d: bool) -> bool {
    if check_2d {
        if node.clone().try_cast::<Node2D>().is_ok() { return true; }
    } else {
        if node.clone().try_cast::<Node3D>().is_ok() { return true; }
    }
    let count = node.get_child_count();
    for i in 0..count {
        if let Some(child) = node.get_child(i) {
            if Self::has_node_type_recursive(&child, check_2d) {
                return true;
            }
        }
    }
    false
}
```

**Implementation Notes:**
- Detection runs once during handshake, not per-frame — performance is not critical
- The recursive scan stops at first match (early return) for efficiency
- Stage's own nodes (StageRuntime etc.) are Node, not Node2D/Node3D, so they don't interfere
- Default to 3D when no spatial nodes exist (matches current behavior)

**Acceptance Criteria:**
- [ ] Scene with only Node2D subtrees → `scene_dimensions: 2`
- [ ] Scene with only Node3D subtrees → `scene_dimensions: 3`
- [ ] Scene with both → `scene_dimensions: 0` (mixed)
- [ ] Empty/plain-Node scene → defaults to `3`

---

### Unit 3: 2D Entity Collection in stage-godot

**File:** `crates/stage-godot/src/collector.rs`

Modify `collect_entities_recursive` to collect both Node3D and Node2D entities:

```rust
fn collect_entities_recursive(
    &self,
    node: &Gd<Node>,
    params: &GetSnapshotDataParams,
    entities: &mut Vec<EntityData>,
) {
    if self.is_stage_node(node) {
        return;
    }

    // Try 3D first, then 2D
    if let Ok(node3d) = node.clone().try_cast::<Node3D>() {
        if self.should_collect_3d(&node3d, params) {
            let entity = self.collect_single_entity_3d(&node3d, params);
            entities.push(entity);
        }
    } else if let Ok(node2d) = node.clone().try_cast::<Node2D>() {
        if self.should_collect_2d(&node2d, params) {
            let entity = self.collect_single_entity_2d(&node2d, params);
            entities.push(entity);
        }
    }

    let count = node.get_child_count();
    for i in 0..count {
        if let Some(child) = node.get_child(i) {
            self.collect_entities_recursive(&child, params, entities);
        }
    }
}
```

Rename existing `should_collect` → `should_collect_3d`, add `should_collect_2d` (same logic but takes `Gd<Node2D>`):

```rust
fn should_collect_2d(&self, node: &Gd<Node2D>, params: &GetSnapshotDataParams) -> bool {
    let class_name = node.get_class().to_string();
    if !params.class_filter.is_empty() {
        if !params.class_filter.iter().any(|f| class_name == *f) {
            return false;
        }
    }
    if !params.groups.is_empty() {
        let node_ref: Gd<Node> = node.clone().upcast();
        let has_matching_group = params
            .groups
            .iter()
            .any(|g| node_ref.is_in_group(g.as_str()));
        if !has_matching_group {
            return false;
        }
    }
    true
}
```

Rename existing `collect_single_entity` → `collect_single_entity_3d`, add `collect_single_entity_2d`:

```rust
fn collect_single_entity_2d(&self, node: &Gd<Node2D>, params: &GetSnapshotDataParams) -> EntityData {
    let pos = node.get_global_position();
    let rot = node.get_global_rotation_degrees();
    let class_name = node.get_class().to_string();
    let node_ref: Gd<Node> = node.clone().upcast();

    let velocity = self.get_velocity_2d(node);
    let groups = self.get_groups(&node_ref);
    let visible = node.is_visible_in_tree();
    let state = self.get_exported_state(&node_ref);

    let mut entity = EntityData {
        path: self.get_relative_path(&node_ref),
        class: class_name,
        position: vec![pos.x as f64, pos.y as f64],
        rotation_deg: vec![rot as f64],
        velocity,
        groups,
        visible,
        state,
        signals_recent: Vec::new(),
        children: Vec::new(),
        script: None,
        signals_connected: Vec::new(),
        physics: None,
        transform: None,
        all_exported_vars: None,
    };

    if params.detail == DetailLevel::Full {
        entity.children = self.get_children(&node_ref);
        entity.script = self.get_script_path(&node_ref);
        entity.signals_connected = self.get_connected_signals(&node_ref);
        entity.physics = self.get_physics_data_2d(node);
        entity.transform = Some(self.get_transform_data_2d(node));
        entity.all_exported_vars = Some(self.get_exported_state(&node_ref));
    }

    entity
}
```

Add 2D velocity extraction:

```rust
fn get_velocity_2d(&self, node: &Gd<Node2D>) -> Vec<f64> {
    if let Ok(body) = node.clone().try_cast::<godot::classes::CharacterBody2D>() {
        let v = body.get_velocity();
        return vec![v.x as f64, v.y as f64];
    }
    if let Ok(body) = node.clone().try_cast::<godot::classes::RigidBody2D>() {
        let v = body.get_linear_velocity();
        return vec![v.x as f64, v.y as f64];
    }
    vec![0.0, 0.0]
}
```

Add 2D physics data:

```rust
fn get_physics_data_2d(&self, node: &Gd<Node2D>) -> Option<PhysicsEntityData> {
    if let Ok(body) = node.clone().try_cast::<godot::classes::CharacterBody2D>() {
        let v = body.get_velocity();
        let on_floor = body.is_on_floor();
        let floor_normal = if on_floor {
            let n = body.get_floor_normal();
            Some(vec![n.x as f64, n.y as f64])
        } else {
            None
        };
        let phys: Gd<godot::classes::PhysicsBody2D> = body.upcast();
        return Some(PhysicsEntityData {
            velocity: vec![v.x as f64, v.y as f64],
            on_floor,
            floor_normal,
            collision_layer: phys.get_collision_layer(),
            collision_mask: phys.get_collision_mask(),
        });
    }
    None
}
```

Add 2D transform data:

```rust
fn get_transform_data_2d(&self, node: &Gd<Node2D>) -> TransformEntityData {
    let t = node.get_global_transform();
    let origin = t.origin;
    let scale = node.get_scale();
    let rot = node.get_global_rotation_degrees();
    TransformEntityData {
        origin: vec![origin.x as f64, origin.y as f64],
        basis: vec![vec![rot as f64]],  // single angle for 2D
        scale: vec![scale.x as f64, scale.y as f64],
    }
}
```

Add new imports at top of collector.rs:

```rust
use godot::classes::{
    CharacterBody2D, CharacterBody3D, Camera2D, Engine, NavigationServer3D,
    Node, Node2D, Node3D, PhysicsBody2D, PhysicsBody3D,
    PhysicsRayQueryParameters2D, PhysicsRayQueryParameters3D,
    PhysicsServer2D, PhysicsServer3D, Resource, RigidBody2D, RigidBody3D,
};
```

**Implementation Notes:**
- The `else if` branch in `collect_entities_recursive` means a Node3D is never also collected as Node2D (Node3D inherits from Node, not Node2D)
- `should_collect_2d` duplicates the filter logic from `should_collect_3d` — accept this small duplication rather than introducing a generic helper (follows project anti-patterns guidance)
- 2D rotation is a single `f64` in the `rotation_deg` vec (length 1), not `[x, y, z]`
- 2D velocity is `[x, y]` (length 2)
- Floor normal in 2D is `[x, y]` (Vector2)

**Acceptance Criteria:**
- [ ] `collect_entities_recursive` collects both Node3D and Node2D entities
- [ ] Node2D entities have `position: [x, y]`, `rotation_deg: [angle]`, `velocity: [x, y]`
- [ ] 2D physics data includes on_floor, collision_layer/mask from CharacterBody2D
- [ ] 2D transform data includes origin `[x, y]`, single rotation angle, scale `[x, y]`
- [ ] Group and class filters work on 2D entities
- [ ] Full detail collects children, script, signals for 2D nodes

---

### Unit 4: 2D Perspective Resolution and Spatial Context

**File:** `crates/stage-godot/src/collector.rs`

Update `resolve_perspective` to handle Camera2D:

```rust
fn resolve_perspective(&self, param: &PerspectiveParam) -> PerspectiveData {
    match param {
        PerspectiveParam::Camera => {
            if let Some(vp) = self.base().get_viewport() {
                // Try 3D camera first
                if let Some(camera) = vp.get_camera_3d() {
                    let pos = camera.get_global_position();
                    let rot = camera.get_global_rotation_degrees();
                    let fwd = camera.get_global_transform().basis.col_c();
                    return PerspectiveData {
                        position: vec3(pos),
                        rotation_deg: vec3(rot),
                        forward: vec3(-fwd),
                    };
                }
                // Try 2D camera
                if let Some(camera) = vp.get_camera_2d() {
                    let pos = camera.get_global_position();
                    let rot = camera.get_global_rotation_degrees();
                    // 2D forward: facing right (+X) by default, rotate by camera angle
                    let rad = (rot as f64).to_radians();
                    let fx = rad.cos();
                    let fy = rad.sin();
                    return PerspectiveData {
                        position: vec![pos.x as f64, pos.y as f64],
                        rotation_deg: vec![rot as f64],
                        forward: vec![fx, fy],
                    };
                }
            }
            // Fallback: no camera found
            PerspectiveData {
                position: vec![0.0, 0.0, 0.0],
                rotation_deg: vec![0.0, 0.0, 0.0],
                forward: vec![0.0, 0.0, -1.0],
            }
        }
        PerspectiveParam::Node { path } => {
            // Try Node3D
            if let Some(node) = self.base().try_get_node_as::<Node3D>(path.as_str()) {
                let pos = node.get_global_position();
                let rot = node.get_global_rotation_degrees();
                let fwd = node.get_global_transform().basis.col_c();
                return PerspectiveData {
                    position: vec3(pos),
                    rotation_deg: vec3(rot),
                    forward: vec3(-fwd),
                };
            }
            // Try Node2D
            if let Some(node) = self.base().try_get_node_as::<Node2D>(path.as_str()) {
                let pos = node.get_global_position();
                let rot = node.get_global_rotation_degrees();
                let rad = (rot as f64).to_radians();
                return PerspectiveData {
                    position: vec![pos.x as f64, pos.y as f64],
                    rotation_deg: vec![rot as f64],
                    forward: vec![rad.cos(), rad.sin()],
                };
            }
            // Fallback
            PerspectiveData {
                position: vec![0.0, 0.0, 0.0],
                rotation_deg: vec![0.0, 0.0, 0.0],
                forward: vec![0.0, 0.0, -1.0],
            }
        }
        PerspectiveParam::Point { position } => {
            let forward = if position.len() == 2 {
                vec![1.0, 0.0]  // 2D default: facing right
            } else {
                vec![0.0, 0.0, -1.0]  // 3D default: facing -Z
            };
            PerspectiveData {
                position: position.clone(),
                rotation_deg: if position.len() == 2 { vec![0.0] } else { vec![0.0, 0.0, 0.0] },
                forward,
            }
        },
    }
}
```

Update `collect_spatial_context_raw` to handle Node2D:

```rust
fn collect_spatial_context_raw(&self, node: &Gd<Node>) -> SpatialContextRaw {
    // Try Node3D
    if let Ok(node3d) = node.clone().try_cast::<Node3D>() {
        return self.collect_spatial_context_raw_3d(&node3d);
    }
    // Try Node2D
    if let Ok(node2d) = node.clone().try_cast::<Node2D>() {
        return self.collect_spatial_context_raw_2d(&node2d);
    }
    // Fallback
    SpatialContextRaw {
        nearby: Vec::new(),
        in_areas: Vec::new(),
        camera_visible: false,
        camera_distance: 0.0,
        node_position: Vec::new(),
        node_forward: Vec::new(),
    }
}
```

Extract existing 3D logic into `collect_spatial_context_raw_3d`, add `collect_spatial_context_raw_2d`:

```rust
fn collect_spatial_context_raw_2d(&self, node: &Gd<Node2D>) -> SpatialContextRaw {
    let pos = node.get_global_position();
    let rot = node.get_global_rotation_degrees();
    let rad = (rot as f64).to_radians();
    let node_position = vec![pos.x as f64, pos.y as f64];
    let node_forward = vec![rad.cos(), rad.sin()];

    let (camera_visible, camera_distance) = if let Some(vp) = self.base().get_viewport() {
        if let Some(camera) = vp.get_camera_2d() {
            let cam_pos = camera.get_global_position();
            let dist = pos.distance_to(cam_pos) as f64;
            let visible = node.is_visible_in_tree();
            (visible, dist)
        } else {
            (false, 0.0)
        }
    } else {
        (false, 0.0)
    };

    let mut nearby = Vec::new();
    if let Some(tree) = self.base().get_tree() {
        if let Some(root) = tree.get_current_scene() {
            self.collect_nearby_recursive_2d(&root, &pos, node.clone().upcast(), &mut nearby, 500.0);
            nearby.sort_by(|a, b| {
                let da = position_distance_2d(&a.position, &node_position);
                let db = position_distance_2d(&b.position, &node_position);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
            nearby.truncate(10);
        }
    }

    let in_areas = self.collect_containing_areas_2d(node);

    SpatialContextRaw {
        nearby,
        in_areas,
        camera_visible,
        camera_distance,
        node_position,
        node_forward,
    }
}
```

Add `collect_nearby_recursive_2d`:

```rust
fn collect_nearby_recursive_2d(
    &self,
    node: &Gd<Node>,
    target_pos: &Vector2,
    exclude: Gd<Node>,
    result: &mut Vec<NearbyEntityRaw>,
    radius: f64,
) {
    if self.is_stage_node(node) { return; }
    if node.instance_id() == exclude.instance_id() { return; }

    if let Ok(n2d) = node.clone().try_cast::<Node2D>() {
        let pos = n2d.get_global_position();
        let dist = pos.distance_to(*target_pos) as f64;
        if dist <= radius {
            let node_ref: Gd<Node> = n2d.clone().upcast();
            result.push(NearbyEntityRaw {
                path: self.get_relative_path(&node_ref),
                class: node_ref.get_class().to_string(),
                position: vec![pos.x as f64, pos.y as f64],
                groups: self.get_groups(&node_ref),
            });
        }
    }

    let count = node.get_child_count();
    for i in 0..count {
        if let Some(child) = node.get_child(i) {
            self.collect_nearby_recursive_2d(&child, target_pos, exclude.clone(), result, radius);
        }
    }
}
```

Add `collect_containing_areas_2d`:

```rust
fn collect_containing_areas_2d(&self, node: &Gd<Node2D>) -> Vec<String> {
    // Area2D overlapping bodies detection
    let mut areas = Vec::new();
    if let Some(tree) = self.base().get_tree() {
        let area_nodes = tree.get_nodes_in_group("".into()); // Fallback: scan tree
        // Simpler approach: just return empty for now, matching 3D which also
        // only detects areas through body overlap queries.
        // Full implementation: iterate Area2D nodes, check get_overlapping_bodies()
    }
    areas
}
```

**Implementation Notes:**
- 2D "forward" direction: in 2D Godot, rotation 0° = facing right (+X). Forward vector = `[cos(rot), sin(rot)]`
- 2D nearby radius is larger (500.0 vs 20.0 for 3D) because 2D games typically have larger coordinate spaces
- `collect_containing_areas_2d` can reuse the same tree-scan pattern as 3D, but checking `Area2D` instead of `Area3D`

Also update `collect_inspect_transform` and `collect_inspect_physics` to handle Node2D:

```rust
fn collect_inspect_transform(&self, node: &Gd<Node>) -> InspectTransform {
    if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
        // ... existing 3D logic ...
    } else if let Ok(n2d) = node.clone().try_cast::<Node2D>() {
        let global = n2d.get_global_position();
        let global_rot = n2d.get_global_rotation_degrees();
        let local = n2d.get_position();
        let scale = n2d.get_scale();
        InspectTransform {
            global_origin: vec![global.x as f64, global.y as f64],
            global_rotation_deg: vec![global_rot as f64],
            local_origin: vec![local.x as f64, local.y as f64],
            scale: vec![scale.x as f64, scale.y as f64],
        }
    } else {
        InspectTransform {
            global_origin: vec![],
            global_rotation_deg: vec![],
            local_origin: vec![],
            scale: vec![],
        }
    }
}

fn collect_inspect_physics(&self, node: &Gd<Node>) -> Option<InspectPhysics> {
    // Existing 3D checks...
    if let Ok(body) = node.clone().try_cast::<godot::classes::CharacterBody3D>() {
        // ... existing 3D ...
    }
    if let Ok(body) = node.clone().try_cast::<godot::classes::RigidBody3D>() {
        // ... existing 3D ...
    }
    // 2D checks
    if let Ok(body) = node.clone().try_cast::<godot::classes::CharacterBody2D>() {
        let v = body.get_velocity();
        let speed = (v.x * v.x + v.y * v.y).sqrt() as f64;
        let on_floor = body.is_on_floor();
        let on_wall = body.is_on_wall();
        let on_ceiling = body.is_on_ceiling();
        let floor_normal = if on_floor {
            let n = body.get_floor_normal();
            Some(vec![n.x as f64, n.y as f64])
        } else {
            None
        };
        let phys: Gd<godot::classes::PhysicsBody2D> = body.upcast();
        return Some(InspectPhysics {
            velocity: vec![v.x as f64, v.y as f64],
            speed,
            on_floor,
            on_wall,
            on_ceiling,
            floor_normal,
            collision_layer: phys.get_collision_layer(),
            collision_mask: phys.get_collision_mask(),
        });
    }
    if let Ok(body) = node.clone().try_cast::<godot::classes::RigidBody2D>() {
        let v = body.get_linear_velocity();
        let speed = (v.x * v.x + v.y * v.y).sqrt() as f64;
        let phys: Gd<godot::classes::PhysicsBody2D> = body.upcast();
        return Some(InspectPhysics {
            velocity: vec![v.x as f64, v.y as f64],
            speed,
            on_floor: false,
            on_wall: false,
            on_ceiling: false,
            floor_normal: None,
            collision_layer: phys.get_collision_layer(),
            collision_mask: phys.get_collision_mask(),
        });
    }
    None
}
```

Update `collect_child_props` to handle 2D child types:

```rust
fn collect_child_props(
    &self,
    child: &Gd<Node>,
    class: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut props = serde_json::Map::new();
    match class {
        // Existing 3D cases...
        "CollisionShape3D" => { /* ... */ }
        "MeshInstance3D" => { /* ... */ }
        "NavigationAgent3D" => { /* ... */ }
        "Area3D" => { /* ... */ }
        // 2D cases
        "CollisionShape2D" => {
            if let Ok(cs) = child.clone().try_cast::<godot::classes::CollisionShape2D>() {
                if let Some(shape) = cs.get_shape() {
                    props.insert(
                        "shape".to_string(),
                        serde_json::Value::String(shape.get_class().to_string()),
                    );
                }
            }
        }
        "Sprite2D" => {
            if let Ok(s) = child.clone().try_cast::<Node2D>() {
                props.insert(
                    "visible".to_string(),
                    serde_json::Value::Bool(s.is_visible()),
                );
            }
        }
        "Area2D" => {
            if let Ok(area) = child.clone().try_cast::<godot::classes::Area2D>() {
                let bodies = area.get_overlapping_bodies();
                let names: Vec<serde_json::Value> = (0..bodies.len())
                    .filter_map(|i| {
                        bodies
                            .get(i)
                            .map(|b| serde_json::Value::String(b.get_name().to_string()))
                    })
                    .collect();
                props.insert(
                    "overlapping_bodies".to_string(),
                    serde_json::Value::Array(names),
                );
            }
        }
        _ => {}
    }
    props
}
```

**Acceptance Criteria:**
- [ ] Camera2D perspective returns `[x, y]` position, single-angle rotation, `[fx, fy]` forward
- [ ] Node2D perspective works for `perspective: "node"` with 2D focal node
- [ ] Point perspective with 2-element array returns 2D forward default
- [ ] Spatial context for Node2D nodes collects nearby Node2D entities
- [ ] Inspect transform for Node2D returns `[x, y]` global/local origins
- [ ] Inspect physics for CharacterBody2D returns on_floor/on_wall/on_ceiling, `[x, y]` velocity
- [ ] Child props for CollisionShape2D, Sprite2D, Area2D are collected

---

### Unit 5: 2D Bearing System in stage-core

**File:** `crates/stage-core/src/bearing.rs`

Add 2D bearing functions alongside existing 3D ones:

```rust
/// Compute the relative position of a 2D target from a 2D perspective.
/// No elevation in 2D.
pub fn relative_position_2d(
    perspective_pos: [f64; 2],
    perspective_forward: [f64; 2],
    target: [f64; 2],
    occluded: bool,
) -> RelativePosition {
    let dist = distance_2d(perspective_pos, target);
    let bdeg = bearing_deg_2d(perspective_pos, perspective_forward, target);
    let bearing = to_cardinal(bdeg);

    RelativePosition {
        dist,
        bearing,
        bearing_deg: bdeg,
        elevation: None,  // No elevation in 2D
        occluded,
    }
}

/// 2D Euclidean distance.
pub fn distance_2d(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

/// 2D bearing angle in degrees from perspective forward to target.
/// 0 = ahead, clockwise.
/// Projects onto XY plane (2D Godot convention: X right, Y down).
pub fn bearing_deg_2d(
    perspective_pos: [f64; 2],
    perspective_forward: [f64; 2],
    target: [f64; 2],
) -> f64 {
    let dx = target[0] - perspective_pos[0];
    let dy = target[1] - perspective_pos[1];

    let fx = perspective_forward[0];
    let fy = perspective_forward[1];

    // 2D cross product: fx*dy - fy*dx (positive = target is clockwise)
    let cross = fx * dy - fy * dx;
    let dot = fx * dx + fy * dy;

    let angle = cross.atan2(dot).to_degrees();
    ((angle % 360.0) + 360.0) % 360.0
}

/// Build a 2D perspective from position and rotation angle (degrees).
/// Godot 2D: 0° = facing right (+X), positive = clockwise.
pub fn perspective_from_angle_2d(position: [f64; 2], angle_deg: f64) -> ([f64; 2], [f64; 2]) {
    let rad = angle_deg.to_radians();
    let forward = [rad.cos(), rad.sin()];
    (position, forward)
}
```

**File:** `crates/stage-core/src/types.rs`

Add the 2D position type alias:

```rust
/// A 2D position in world space.
pub type Position2 = [f64; 2];

/// Convert a `Vec<f64>` slice to a fixed `[f64; 2]`, filling missing elements with `0.0`.
pub fn vec_to_array2(v: &[f64]) -> [f64; 2] {
    [
        v.first().copied().unwrap_or(0.0),
        v.get(1).copied().unwrap_or(0.0),
    ]
}
```

**Implementation Notes:**
- 2D Godot coordinate system: X right, Y down. Rotation 0° = facing right (+X), positive rotation = clockwise
- The `to_cardinal` function is reused for both 2D and 3D (it's just degree→octant mapping)
- `RelativePosition.elevation` is `None` for 2D entities (already `Option<Elevation>` with `skip_serializing_if`)
- No changes needed to `Cardinal` or `Elevation` enums

**Acceptance Criteria:**
- [ ] `distance_2d([0, 0], [3, 4])` returns 5.0
- [ ] `bearing_deg_2d` with forward `[1, 0]` and target directly ahead returns ~0°
- [ ] `bearing_deg_2d` with forward `[1, 0]` and target to the right (+Y in Godot 2D) returns ~90°
- [ ] `relative_position_2d` returns `elevation: None`
- [ ] `vec_to_array2` correctly converts slices

---

### Unit 6: 2D Grid Hash Spatial Index

**File:** `crates/stage-core/src/index.rs`

Add a `GridHash2D` struct and make `SpatialIndex` an enum:

```rust
use crate::types::{Position2, Position3};

/// 2D grid hash cell size (world units).
const GRID_CELL_SIZE: f64 = 64.0;

/// Spatial index — either 3D R-tree or 2D grid hash.
pub enum SpatialIndex {
    /// 3D R-tree spatial index (rstar).
    Three(SpatialIndex3D),
    /// 2D grid hash spatial index.
    Two(GridHash2D),
}

/// R-tree spatial index for 3D (existing implementation, renamed).
pub struct SpatialIndex3D {
    tree: RTree<IndexedEntity>,
}

/// Grid hash for 2D spatial indexing.
pub struct GridHash2D {
    cells: HashMap<(i64, i64), Vec<IndexedEntity2D>>,
    all: Vec<IndexedEntity2D>,
    cell_size: f64,
}

/// A 2D indexed entity.
#[derive(Debug, Clone)]
pub struct IndexedEntity2D {
    pub path: String,
    pub class: String,
    pub position: Position2,
    pub groups: Vec<String>,
}

impl GridHash2D {
    pub fn build(entities: Vec<IndexedEntity2D>, cell_size: f64) -> Self {
        let mut cells: HashMap<(i64, i64), Vec<IndexedEntity2D>> = HashMap::new();
        for entity in &entities {
            let key = Self::cell_key(entity.position, cell_size);
            cells.entry(key).or_default().push(entity.clone());
        }
        Self { cells, all: entities, cell_size }
    }

    fn cell_key(pos: Position2, cell_size: f64) -> (i64, i64) {
        (
            (pos[0] / cell_size).floor() as i64,
            (pos[1] / cell_size).floor() as i64,
        )
    }

    pub fn nearest(
        &self,
        point: Position2,
        k: usize,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        let mut results: Vec<NearestResult> = self
            .all
            .iter()
            .filter(|e| Self::matches_filters(e, groups, class_filter))
            .map(|e| NearestResult {
                path: e.path.clone(),
                class: e.class.clone(),
                position: [e.position[0], e.position[1], 0.0],
                distance: crate::bearing::distance_2d(point, e.position),
                groups: e.groups.clone(),
            })
            .collect();
        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    pub fn within_radius(
        &self,
        point: Position2,
        radius: f64,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        let r2 = radius * radius;
        // Calculate cell range to check
        let min_cx = ((point[0] - radius) / self.cell_size).floor() as i64;
        let max_cx = ((point[0] + radius) / self.cell_size).floor() as i64;
        let min_cy = ((point[1] - radius) / self.cell_size).floor() as i64;
        let max_cy = ((point[1] + radius) / self.cell_size).floor() as i64;

        let mut results: Vec<NearestResult> = Vec::new();
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(entities) = self.cells.get(&(cx, cy)) {
                    for e in entities {
                        let dx = e.position[0] - point[0];
                        let dy = e.position[1] - point[1];
                        let d2 = dx * dx + dy * dy;
                        if d2 <= r2 && Self::matches_filters(e, groups, class_filter) {
                            results.push(NearestResult {
                                path: e.path.clone(),
                                class: e.class.clone(),
                                position: [e.position[0], e.position[1], 0.0],
                                distance: d2.sqrt(),
                                groups: e.groups.clone(),
                            });
                        }
                    }
                }
            }
        }
        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    fn matches_filters(entity: &IndexedEntity2D, groups: &[String], class_filter: &[String]) -> bool {
        let group_ok = groups.is_empty() || entity.groups.iter().any(|g| groups.contains(g));
        let class_ok = class_filter.is_empty() || class_filter.iter().any(|c| c == &entity.class);
        group_ok && class_ok
    }

    pub fn len(&self) -> usize { self.all.len() }
    pub fn is_empty(&self) -> bool { self.all.is_empty() }
}
```

Update `SpatialIndex` to delegate to the inner variant:

```rust
impl SpatialIndex {
    /// Build a 3D index.
    pub fn build(entities: Vec<IndexedEntity>) -> Self {
        Self::Three(SpatialIndex3D::build(entities))
    }

    /// Build a 2D index.
    pub fn build_2d(entities: Vec<IndexedEntity2D>) -> Self {
        Self::Two(GridHash2D::build(entities, GRID_CELL_SIZE))
    }

    /// Create an empty 3D index (default).
    pub fn empty() -> Self {
        Self::Three(SpatialIndex3D::empty())
    }

    pub fn nearest(
        &self,
        point: Position3,
        k: usize,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        match self {
            Self::Three(idx) => idx.nearest(point, k, groups, class_filter),
            Self::Two(idx) => idx.nearest([point[0], point[1]], k, groups, class_filter),
        }
    }

    pub fn within_radius(
        &self,
        point: Position3,
        radius: f64,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        match self {
            Self::Three(idx) => idx.within_radius(point, radius, groups, class_filter),
            Self::Two(idx) => idx.within_radius([point[0], point[1]], radius, groups, class_filter),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Three(idx) => idx.len(),
            Self::Two(idx) => idx.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Three(idx) => idx.is_empty(),
            Self::Two(idx) => idx.is_empty(),
        }
    }
}
```

Move existing R-tree methods into `SpatialIndex3D`:

```rust
impl SpatialIndex3D {
    pub fn build(entities: Vec<IndexedEntity>) -> Self {
        Self { tree: RTree::bulk_load(entities) }
    }

    pub fn empty() -> Self {
        Self { tree: RTree::new() }
    }

    pub fn nearest(&self, point: Position3, k: usize, groups: &[String], class_filter: &[String]) -> Vec<NearestResult> {
        // ... existing logic from SpatialIndex::nearest ...
    }

    pub fn within_radius(&self, point: Position3, radius: f64, groups: &[String], class_filter: &[String]) -> Vec<NearestResult> {
        // ... existing logic from SpatialIndex::within_radius ...
    }

    fn matches_filters(entity: &IndexedEntity, groups: &[String], class_filter: &[String]) -> bool {
        // ... existing logic ...
    }

    pub fn len(&self) -> usize { self.tree.size() }
    pub fn is_empty(&self) -> bool { self.tree.size() == 0 }
}
```

Add `use std::collections::HashMap;` to imports.

**Implementation Notes:**
- `NearestResult.position` remains `Position3` to keep the server-side API uniform — 2D entities set `z = 0.0`
- The grid hash `within_radius` scans only cells that overlap the query circle — O(cells_in_radius * entities_per_cell) which is fast for typical 2D scene densities
- `nearest` on `GridHash2D` scans all entities (O(n) with sort) — acceptable for < 500 dynamic entities. More sophisticated approaches (expanding ring search) are unnecessary given typical scene sizes.
- Cell size of 64 units works well for most 2D games (tiles are typically 16-64 pixels)

**Acceptance Criteria:**
- [ ] `SpatialIndex::build()` creates a 3D R-tree (existing behavior)
- [ ] `SpatialIndex::build_2d()` creates a 2D grid hash
- [ ] `SpatialIndex::nearest()` delegates to the correct inner index
- [ ] `SpatialIndex::within_radius()` delegates correctly
- [ ] Grid hash `nearest` returns K closest 2D entities
- [ ] Grid hash `within_radius` returns entities within radius, sorted by distance
- [ ] Group and class filters work on 2D grid hash
- [ ] All existing 3D `SpatialIndex` tests pass unchanged

---

### Unit 7: Dimension-Aware Server Layer

**File:** `crates/stage-server/src/tcp.rs`

Store `scene_dimensions` in `SessionState` and use it:

```rust
use stage_protocol::handshake::SceneDimensions;

pub struct SessionState {
    // ... existing fields ...
    /// Scene dimensions from handshake.
    pub scene_dimensions: SceneDimensions,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            scene_dimensions: SceneDimensions::Three,
        }
    }
}
```

In `handle_connection`, after receiving handshake, set `scene_dimensions`:

```rust
// After handshake parsing:
s.scene_dimensions = handshake.dimensions();
```

**File:** `crates/stage-server/src/mcp/snapshot.rs`

Update `build_perspective` to handle 2D:

```rust
pub fn build_perspective(data: &PerspectiveData) -> Perspective {
    if data.position.len() == 2 {
        // 2D perspective: pad position to [x, y, 0], forward to [fx, fy, 0]
        let position: Position3 = [data.position[0], data.position[1], 0.0];
        let forward = [
            data.forward.first().copied().unwrap_or(1.0),
            data.forward.get(1).copied().unwrap_or(0.0),
            0.0,
        ];
        let (facing, facing_deg) = bearing::compass_bearing(forward);
        Perspective { position, forward, facing, facing_deg }
    } else {
        // Existing 3D logic
        let position: Position3 = [data.position[0], data.position[1], data.position[2]];
        let forward = [data.forward[0], data.forward[1], data.forward[2]];
        let (facing, facing_deg) = bearing::compass_bearing(forward);
        Perspective { position, forward, facing, facing_deg }
    }
}
```

Update `build_output_entity` to output `rot` instead of `rot_y` for 2D:

```rust
pub fn build_output_entity(
    entity: &EntityData,
    rel: &RelativePosition,
    full: bool,
    config: &SessionConfig,
) -> OutputEntity {
    // ... existing logic ...

    let is_2d = entity.position.len() == 2;

    OutputEntity {
        // ... existing fields ...
        rot_y: if is_2d { None } else { entity.rotation_deg.get(1).copied() },
        rot: if is_2d { entity.rotation_deg.first().copied() } else { None },
        // ...
    }
}
```

Add `rot` field to `OutputEntity`:

```rust
pub struct OutputEntity {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot_y: Option<f64>,
    /// 2D rotation angle in degrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot: Option<f64>,
    // ...
}
```

Update snapshot response building to compute bearings dimension-appropriately. In the main snapshot handler where `relative_position` is called:

```rust
// Determine if entity is 2D based on position vector length
let rel = if entity.position.len() == 2 {
    bearing::relative_position_2d(
        [perspective.position[0], perspective.position[1]],
        [perspective.forward[0], perspective.forward[1]],
        vec_to_array2(&entity.position),
        !entity.visible,
    )
} else {
    bearing::relative_position(
        &perspective,
        vec_to_array3(&entity.position),
        !entity.visible,
    )
};
```

Update spatial index building to use 2D index for 2D scenes:

```rust
// In the snapshot handler, after collecting entities, when rebuilding the spatial index:
let new_index = if scene_dimensions.is_2d() {
    let indexed: Vec<IndexedEntity2D> = dynamic_entities
        .iter()
        .map(|(e, _)| IndexedEntity2D {
            path: e.path.clone(),
            class: e.class.clone(),
            position: vec_to_array2(&e.position),
            groups: e.groups.clone(),
        })
        .collect();
    SpatialIndex::build_2d(indexed)
} else {
    let indexed: Vec<IndexedEntity> = dynamic_entities
        .iter()
        .map(|(e, _)| IndexedEntity {
            path: e.path.clone(),
            class: e.class.clone(),
            position: vec_to_array3(&e.position),
            groups: e.groups.clone(),
        })
        .collect();
    SpatialIndex::build(indexed)
};
```

**File:** `crates/stage-server/src/mcp/mod.rs`

Add a helper to get scene_dimensions from state (for use by tool handlers):

```rust
/// Get scene dimensions from session state. Must be called with lock held.
pub fn scene_dimensions(state: &SessionState) -> SceneDimensions {
    state.scene_dimensions
}
```

**Implementation Notes:**
- The key insight is that `entity.position.len()` tells us whether a specific entity is 2D or 3D, even in mixed scenes
- `Perspective` still uses `Position3` internally (Z=0 for 2D) — this avoids making the Perspective type generic
- `compass_bearing` in 3D uses XZ plane; for 2D we need to handle XY plane. The simplest approach: when 2D data is padded into `[x, y, 0]`, the bearing computation still works because we project onto XZ in 3D which maps to XY when the data is `[x, y_as_z, 0]` — **wait, this is wrong**. For 2D we must compute on the XY plane directly. That's why we call `relative_position_2d` which uses XY-plane math.
- The `compass_bearing` function won't be called for 2D perspectives — instead, the facing is computed from the 2D forward vector directly

**Acceptance Criteria:**
- [ ] `SessionState` stores `scene_dimensions` from handshake
- [ ] `build_perspective` handles 2D perspective data (position len == 2)
- [ ] `OutputEntity` has `rot` field for 2D, `rot_y` for 3D (never both)
- [ ] Bearing computation uses `relative_position_2d` for 2D entities
- [ ] Spatial index is built as `GridHash2D` for 2D scenes
- [ ] 2D entities produce `elevation: None` in `rel` blocks

---

### Unit 8: 2D Delta Engine Adaptation

**File:** `crates/stage-core/src/delta.rs`

The delta engine's `EntitySnapshot` uses `Position3 = [f64; 3]`. For 2D entities, the Z component will be 0.0. The movement detection threshold (0.01) applies to Euclidean distance, which works correctly in 2D when Z=0.

The primary change: the `detect_movement` function already computes Euclidean distance using all 3 components. With Z=0 for 2D entities, this naturally degenerates to 2D distance. **No code changes needed in delta.rs** — the zero-padded Z approach works transparently.

The `to_entity_snapshot` function in `snapshot.rs` uses `vec_to_array3` which already pads short vectors with 0.0. This handles 2D `position: [x, y]` → `[x, y, 0.0]` automatically.

**Implementation Notes:**
- Zero-padding strategy means delta detection works without changes
- Rotation threshold comparison works because it compares individual components — 2D only has `rotation_deg[0]`, indices 1 and 2 are 0.0 (padded)
- This is a deliberate non-change — verifying it works is still an acceptance criterion

**Acceptance Criteria:**
- [ ] 2D entity position changes are detected by delta engine (movement threshold applies to 2D distance)
- [ ] 2D entity rotation changes are detected (single angle comparison)
- [ ] `to_entity_snapshot` correctly pads 2D positions to `[x, y, 0.0]`

---

### Unit 9: 2D Raycast Support

**File:** `crates/stage-godot/src/collector.rs` (or a new method)

The raycast implementation needs to dispatch to 2D or 3D physics. Looking at the current code, raycasts are handled via the `spatial_query` TCP method which calls into the addon. Add 2D raycast handling:

**File:** `crates/stage-godot/src/query_handler.rs`

In the raycast handler (or wherever `SpatialQueryRequest::Raycast` is dispatched):

```rust
// After resolving from/to positions:
fn execute_raycast_2d(
    from: Vector2,
    to: Vector2,
    collision_mask: Option<u32>,
    collector: &StageCollector,
) -> RaycastResponse {
    let total_distance = from.distance_to(to) as f64;

    let space_rid = collector.base().get_viewport()
        .map(|vp| vp.get_world_2d().map(|w| w.get_space()))
        .flatten();

    let Some(space) = space_rid else {
        return RaycastResponse {
            clear: true,
            blocked_by: None,
            blocked_at: None,
            total_distance,
            clear_distance: total_distance,
        };
    };

    let mut physics_server = PhysicsServer2D::singleton();
    let space_state = physics_server.space_get_direct_state(space);
    // ... use PhysicsDirectSpaceState2D to do the raycast ...
    // Similar to 3D but using 2D parameters
}
```

**Implementation Notes:**
- The decision between 2D and 3D raycast is made by checking `scene_dimensions` from the session, or by checking the position vector length
- PhysicsServer2D API mirrors PhysicsServer3D — query parameters use Vector2 instead of Vector3
- `RaycastResponse.blocked_at` will be `[x, y]` for 2D (Vec<f64>)

**Acceptance Criteria:**
- [ ] Raycasts in 2D scenes use `PhysicsServer2D` / `PhysicsRayQueryParameters2D`
- [ ] 2D raycast results have `blocked_at: [x, y]` (2 elements)
- [ ] 3D raycasts continue to work as before

---

### Unit 10: Mixed Scene Support

**File:** Various — this is a cross-cutting concern

For `scene_dimensions: "mixed"`, the system must handle both Node2D and Node3D entities in the same scene:

1. **Collector** (Unit 3): Already handles both — the `collect_entities_recursive` tries Node3D then Node2D
2. **Spatial Index** (Unit 6): Use 3D R-tree for mixed scenes (2D entities get Z=0)
3. **Bearings** (Unit 7): Dispatch based on `entity.position.len()` per-entity
4. **Output** (Unit 7): Each entity gets `rot` or `rot_y` based on its own dimension

The only new code needed:

```rust
// In spatial index building for mixed scenes:
let new_index = if scene_dimensions.is_2d() {
    // ... build 2D index ...
} else {
    // 3D or mixed: use R-tree (2D entities get Z=0)
    let indexed: Vec<IndexedEntity> = dynamic_entities
        .iter()
        .map(|(e, _)| IndexedEntity {
            path: e.path.clone(),
            class: e.class.clone(),
            position: vec_to_array3(&e.position), // pads 2D to [x, y, 0]
            groups: e.groups.clone(),
        })
        .collect();
    SpatialIndex::build(indexed)
};
```

**Acceptance Criteria:**
- [ ] Mixed scenes collect both Node2D and Node3D entities
- [ ] Each entity in a mixed scene has correct coordinate dimensions in output
- [ ] R-tree is used for mixed scene spatial indexing
- [ ] `dimension_mismatch` error returned for 3D-only operations (e.g., elevation query) in 2D context

---

## Implementation Order

1. **Unit 1: SceneDimensions enum** — foundation type, no dependencies
2. **Unit 5: 2D bearing system** — pure math, no dependencies on other units
3. **Unit 6: 2D grid hash spatial index** — pure data structure, no dependencies
4. **Unit 2: Scene dimension detection** — needs Unit 1 enum
5. **Unit 3: 2D entity collection** — needs Unit 2 for detection, uses 2D types
6. **Unit 4: 2D perspective and spatial context** — needs Unit 3 patterns
7. **Unit 8: Delta engine verification** — verify existing code works with 2D data
8. **Unit 7: Dimension-aware server layer** — needs Units 1-6, integrates everything
9. **Unit 9: 2D raycast** — needs Unit 7 for dimension dispatch
10. **Unit 10: Mixed scene support** — needs everything above

---

## Testing

### Unit Tests: `crates/stage-core/src/bearing.rs`

```rust
#[test]
fn distance_2d_basic() {
    assert!((distance_2d([0.0, 0.0], [3.0, 4.0]) - 5.0).abs() < 1e-10);
}

#[test]
fn bearing_2d_ahead() {
    // Facing right (+X), target directly ahead
    let bdeg = bearing_deg_2d([0.0, 0.0], [1.0, 0.0], [10.0, 0.0]);
    assert!(bdeg < 1.0 || bdeg > 359.0, "Expected ~0°, got {bdeg}");
}

#[test]
fn bearing_2d_right() {
    // Facing right (+X), target below (+Y in Godot 2D)
    let bdeg = bearing_deg_2d([0.0, 0.0], [1.0, 0.0], [0.0, 10.0]);
    assert!((bdeg - 90.0).abs() < 1.0, "Expected ~90°, got {bdeg}");
}

#[test]
fn relative_position_2d_no_elevation() {
    let rel = relative_position_2d([0.0, 0.0], [1.0, 0.0], [10.0, 0.0], false);
    assert!(rel.elevation.is_none());
    assert!(rel.dist > 9.9);
}
```

### Unit Tests: `crates/stage-core/src/index.rs`

```rust
#[test]
fn grid_hash_nearest() {
    let entities = vec![
        IndexedEntity2D { path: "a".into(), class: "Node2D".into(), position: [0.0, 0.0], groups: vec![] },
        IndexedEntity2D { path: "b".into(), class: "Node2D".into(), position: [50.0, 0.0], groups: vec![] },
        IndexedEntity2D { path: "c".into(), class: "Node2D".into(), position: [100.0, 0.0], groups: vec![] },
    ];
    let index = SpatialIndex::build_2d(entities);
    let results = index.nearest([0.0, 0.0, 0.0], 2, &[], &[]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].path, "a");
    assert_eq!(results[1].path, "b");
}

#[test]
fn grid_hash_within_radius() {
    let entities = vec![
        IndexedEntity2D { path: "close".into(), class: "Node2D".into(), position: [30.0, 0.0], groups: vec![] },
        IndexedEntity2D { path: "far".into(), class: "Node2D".into(), position: [1000.0, 0.0], groups: vec![] },
    ];
    let index = SpatialIndex::build_2d(entities);
    let results = index.within_radius([0.0, 0.0, 0.0], 100.0, &[], &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "close");
}

#[test]
fn grid_hash_group_filter() {
    let entities = vec![
        IndexedEntity2D { path: "enemy".into(), class: "CharacterBody2D".into(), position: [10.0, 0.0], groups: vec!["enemies".into()] },
        IndexedEntity2D { path: "pickup".into(), class: "Area2D".into(), position: [20.0, 0.0], groups: vec!["pickups".into()] },
    ];
    let index = SpatialIndex::build_2d(entities);
    let results = index.nearest([0.0, 0.0, 0.0], 5, &["enemies".to_string()], &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "enemy");
}

#[test]
fn spatial_index_3d_still_works() {
    // Verify existing 3D SpatialIndex::build still works after refactor to enum
    let entities = vec![
        IndexedEntity { path: "a".into(), class: "Node3D".into(), position: [0.0, 0.0, 0.0], groups: vec![] },
        IndexedEntity { path: "b".into(), class: "Node3D".into(), position: [5.0, 0.0, 0.0], groups: vec![] },
    ];
    let index = SpatialIndex::build(entities);
    let results = index.nearest([0.0, 0.0, 0.0], 1, &[], &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "a");
}
```

### Unit Tests: `crates/stage-protocol/src/handshake.rs`

```rust
#[test]
fn scene_dimensions_from_u32() {
    assert_eq!(SceneDimensions::from_u32(2), SceneDimensions::Two);
    assert_eq!(SceneDimensions::from_u32(3), SceneDimensions::Three);
    assert_eq!(SceneDimensions::from_u32(0), SceneDimensions::Mixed);
    assert_eq!(SceneDimensions::from_u32(99), SceneDimensions::Mixed);
}

#[test]
fn scene_dimensions_predicates() {
    assert!(SceneDimensions::Two.is_2d());
    assert!(!SceneDimensions::Two.is_3d());
    assert!(SceneDimensions::Three.is_3d());
    assert!(SceneDimensions::Mixed.is_mixed());
}
```

### Unit Tests: `crates/stage-protocol/src/static_classes.rs`

```rust
#[test]
fn static_body_2d_is_static() {
    assert!(is_static_class("StaticBody2D"));
}

#[test]
fn tilemap_layer_is_static() {
    assert!(is_static_class("TileMapLayer"));
}

#[test]
fn classify_2d_classes() {
    assert_eq!(classify_static_category("StaticBody2D"), "collision");
    assert_eq!(classify_static_category("TileMapLayer"), "tilemap");
    assert_eq!(classify_static_category("Sprite2D"), "visual");
    assert_eq!(classify_static_category("PointLight2D"), "lights");
}
```

### Integration Tests: `crates/stage-core/src/delta.rs`

```rust
#[test]
fn delta_detects_2d_movement() {
    let mut engine = DeltaEngine::new();
    let entities_t0 = vec![EntitySnapshot {
        path: "player".into(),
        class: "CharacterBody2D".into(),
        position: [100.0, 200.0, 0.0],  // zero-padded 2D
        rotation_deg: [45.0, 0.0, 0.0],
        velocity: [5.0, 0.0, 0.0],
        groups: vec![],
        state: serde_json::Map::new(),
        visible: true,
    }];
    engine.update_baseline(&entities_t0, 1);

    let entities_t1 = vec![EntitySnapshot {
        path: "player".into(),
        class: "CharacterBody2D".into(),
        position: [105.0, 200.0, 0.0],
        rotation_deg: [45.0, 0.0, 0.0],
        velocity: [5.0, 0.0, 0.0],
        groups: vec![],
        state: serde_json::Map::new(),
        visible: true,
    }];
    let result = engine.compute_delta(&entities_t1, 2);
    assert_eq!(result.moved.len(), 1);
    assert_eq!(result.moved[0].path, "player");
}
```

---

## Verification Checklist

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Verify no println! in library code
grep -r 'println!' crates/ --include='*.rs' | grep -v '#\[cfg(test)\]' | grep -v '// '

# Check that 3D tests still pass (regression)
cargo test --workspace -- bearing::tests
cargo test --workspace -- index::tests
cargo test --workspace -- delta::tests

# Check new 2D tests
cargo test --workspace -- bearing_2d
cargo test --workspace -- grid_hash
cargo test --workspace -- scene_dimensions
```
