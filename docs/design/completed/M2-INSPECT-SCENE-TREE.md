# Design: Milestone 2 — Inspect & Scene Tree

## Overview

M2 delivers two high-value MCP tools: `spatial_inspect` for deep single-node investigation, and `scene_tree` for navigating the Godot scene hierarchy. These are independent of the delta/watch/recording systems and build directly on M1's infrastructure.

**Exit Criteria:** Agent calls `spatial_inspect(node: "enemies/scout_02")` → gets transform, physics, state, children, signals, script, spatial_context. Agent calls `scene_tree(action: "find", find_by: "class", find_value: "CharacterBody3D")` → gets all matching nodes. Agent navigates hierarchy with subtree/children/ancestors.

**Depends on:** M1 (TCP query flow, collector, MCP tool registration)

---

## Implementation Units

### Unit 1: Protocol Query Types for Inspect & Scene Tree

**File:** `crates/spectator-protocol/src/query.rs`

Add new request/response types alongside the existing `GetSnapshotDataParams` / `SnapshotResponse`.

```rust
// --- spatial_inspect protocol types ---

/// Parameters for `get_node_inspect` query method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeInspectParams {
    /// Node path relative to scene root.
    pub path: String,
    /// Which data categories to collect.
    pub include: Vec<InspectCategory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectCategory {
    Transform,
    Physics,
    State,
    Children,
    Signals,
    Script,
    SpatialContext,
}

/// Response from `get_node_inspect`.
/// Raw data from the addon — server post-processes spatial_context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInspectResponse {
    pub path: String,
    pub class: String,
    pub instance_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<InspectTransform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physics: Option<InspectPhysics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<InspectState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<InspectChild>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signals: Option<InspectSignals>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<InspectScript>,
    /// Raw nearby-entity data for spatial_context (server computes bearings).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spatial_context_raw: Option<SpatialContextRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectTransform {
    pub global_origin: Vec<f64>,
    pub global_rotation_deg: Vec<f64>,
    pub local_origin: Vec<f64>,
    pub scale: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectPhysics {
    pub velocity: Vec<f64>,
    pub speed: f64,
    pub on_floor: bool,
    pub on_wall: bool,
    pub on_ceiling: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor_normal: Option<Vec<f64>>,
    pub collision_layer: u32,
    pub collision_mask: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectState {
    pub exported: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectChild {
    pub name: String,
    pub class: String,
    /// Key property summaries (e.g., shape info for CollisionShape3D).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub props: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectSignals {
    /// Signal → list of target strings ("node_path:method").
    pub connected: serde_json::Map<String, serde_json::Value>,
    pub recent_emissions: Vec<SignalEmission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalEmission {
    pub signal: String,
    pub frame: u64,
    #[serde(default)]
    pub args: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectScript {
    pub path: String,
    pub base_class: String,
    pub methods: Vec<String>,
    pub extends_chain: Vec<String>,
}

/// Raw spatial context data collected by addon.
/// Server post-processes with bearing calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialContextRaw {
    /// Nearby entities with positions (server computes bearings).
    pub nearby: Vec<NearbyEntityRaw>,
    /// Area3D/Area2D nodes the target is inside.
    pub in_areas: Vec<String>,
    /// Whether the node is visible to the camera.
    pub camera_visible: bool,
    /// Distance from the active camera.
    pub camera_distance: f64,
    /// The target node's position and forward vector (for bearing calc).
    pub node_position: Vec<f64>,
    pub node_forward: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyEntityRaw {
    pub path: String,
    pub class: String,
    pub position: Vec<f64>,
    pub groups: Vec<String>,
}

// --- scene_tree protocol types ---

/// Parameters for `get_scene_tree` query method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSceneTreeParams {
    pub action: SceneTreeAction,
    /// Node path — required for children, subtree, ancestors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Max recursion depth for subtree. Default: 3.
    #[serde(default = "default_depth")]
    pub depth: u32,
    /// For find: search criterion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find_by: Option<FindBy>,
    /// For find: search value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find_value: Option<String>,
    /// What to include per node.
    #[serde(default = "default_tree_include")]
    pub include: Vec<TreeInclude>,
}

fn default_depth() -> u32 {
    3
}

fn default_tree_include() -> Vec<TreeInclude> {
    vec![TreeInclude::Class, TreeInclude::Groups]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneTreeAction {
    Roots,
    Children,
    Subtree,
    Ancestors,
    Find,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindBy {
    Name,
    Class,
    Group,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeInclude {
    Class,
    Groups,
    Script,
    Visible,
    ProcessMode,
}

/// Response for scene_tree queries (generic envelope).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneTreeResponse {
    /// The response payload varies by action.
    pub data: serde_json::Value,
}
```

**Implementation Notes:**
- `SpatialContextRaw` gives the addon's raw nearby data. The server computes bearings from these positions. This keeps bearing logic server-side per architecture rules.
- `SceneTreeResponse` uses a `serde_json::Value` payload because each action has a different response shape. The server formats the MCP output, not the addon.
- `InspectChild.props` carries key properties for important child types (e.g., `CollisionShape3D` → `{ "shape": "CapsuleShape3D(r=0.5, h=1.8)" }`). The addon selectively populates this.
- Signal emission tracking reuses the data the collector already gathers for `signals_recent` but adds `args`.

**Acceptance Criteria:**
- [ ] All new types serialize/deserialize correctly (round-trip tests)
- [ ] `InspectCategory` and `SceneTreeAction` use `rename_all = "snake_case"`
- [ ] Optional fields use `skip_serializing_if`

---

### Unit 2: GDExtension Collector — Node Inspection

**File:** `crates/spectator-godot/src/collector.rs`

Add methods to `SpectatorCollector` for deep node inspection. These extend the existing impl block.

```rust
impl SpectatorCollector {
    /// Collect deep inspection data for a single node.
    pub fn inspect_node(&self, params: &GetNodeInspectParams) -> Result<NodeInspectResponse, String> {
        let tree = self.base().get_tree()
            .ok_or("No scene tree available")?;
        let root = tree.get_current_scene()
            .ok_or("No current scene")?;
        let node: Gd<Node> = root.try_get_node_as(params.path.as_str())
            .ok_or_else(|| format!("Node '{}' not found", params.path))?;

        let class = node.get_class().to_string();
        let instance_id = node.instance_id().to_i64() as u64;

        let mut response = NodeInspectResponse {
            path: params.path.clone(),
            class,
            instance_id,
            transform: None,
            physics: None,
            state: None,
            children: None,
            signals: None,
            script: None,
            spatial_context_raw: None,
        };

        let is_3d = node.clone().try_cast::<Node3D>().is_ok();

        for cat in &params.include {
            match cat {
                InspectCategory::Transform => {
                    response.transform = Some(self.collect_inspect_transform(&node));
                }
                InspectCategory::Physics => {
                    response.physics = self.collect_inspect_physics(&node);
                }
                InspectCategory::State => {
                    response.state = Some(self.collect_inspect_state(&node));
                }
                InspectCategory::Children => {
                    response.children = Some(self.collect_inspect_children(&node));
                }
                InspectCategory::Signals => {
                    response.signals = Some(self.collect_inspect_signals(&node));
                }
                InspectCategory::Script => {
                    response.script = self.collect_inspect_script(&node);
                }
                InspectCategory::SpatialContext => {
                    if is_3d {
                        response.spatial_context_raw =
                            Some(self.collect_spatial_context_raw(&node));
                    }
                }
            }
        }

        Ok(response)
    }

    fn collect_inspect_transform(&self, node: &Gd<Node>) -> InspectTransform {
        // Try Node3D first, fall back to zero transform
        if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
            let global = n3d.get_global_position();
            let global_rot = n3d.get_global_rotation_degrees();
            let local = n3d.get_position();
            let scale = n3d.get_scale();
            InspectTransform {
                global_origin: vec![global.x as f64, global.y as f64, global.z as f64],
                global_rotation_deg: vec![global_rot.x as f64, global_rot.y as f64, global_rot.z as f64],
                local_origin: vec![local.x as f64, local.y as f64, local.z as f64],
                scale: vec![scale.x as f64, scale.y as f64, scale.z as f64],
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
        // CharacterBody3D
        if let Ok(body) = node.clone().try_cast::<CharacterBody3D>() {
            let v = body.get_velocity();
            let speed = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt() as f64;
            let on_floor = body.is_on_floor();
            let floor_normal = if on_floor {
                let n = body.get_floor_normal();
                Some(vec![n.x as f64, n.y as f64, n.z as f64])
            } else {
                None
            };
            let phys: Gd<PhysicsBody3D> = body.upcast();
            return Some(InspectPhysics {
                velocity: vec![v.x as f64, v.y as f64, v.z as f64],
                speed,
                on_floor,
                on_wall: false, // CharacterBody3D.is_on_wall() — add
                on_ceiling: false, // CharacterBody3D.is_on_ceiling() — add
                floor_normal,
                collision_layer: phys.get_collision_layer(),
                collision_mask: phys.get_collision_mask(),
            });
        }
        // RigidBody3D
        if let Ok(body) = node.clone().try_cast::<RigidBody3D>() {
            let v = body.get_linear_velocity();
            let speed = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt() as f64;
            let phys: Gd<PhysicsBody3D> = body.upcast();
            return Some(InspectPhysics {
                velocity: vec![v.x as f64, v.y as f64, v.z as f64],
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

    fn collect_inspect_state(&self, node: &Gd<Node>) -> InspectState {
        InspectState {
            exported: self.get_exported_state(node), // reuse existing method
            internal: None, // expose_internals is a server config — addon doesn't filter
        }
    }

    fn collect_inspect_children(&self, node: &Gd<Node>) -> Vec<InspectChild> {
        let count = node.get_child_count();
        let mut children = Vec::new();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                let class = child.get_class().to_string();
                let props = self.collect_child_props(&child, &class);
                children.push(InspectChild {
                    name: child.get_name().to_string(),
                    class,
                    props,
                });
            }
        }
        children
    }

    /// Collect key properties for notable child types.
    fn collect_child_props(
        &self,
        child: &Gd<Node>,
        class: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut props = serde_json::Map::new();
        match class {
            "CollisionShape3D" => {
                // Get shape description
                if let Ok(cs) = child.clone().try_cast::<godot::classes::CollisionShape3D>() {
                    if let Some(shape) = cs.get_shape() {
                        props.insert(
                            "shape".to_string(),
                            serde_json::Value::String(format!("{}", shape.get_class())),
                        );
                    }
                }
            }
            "MeshInstance3D" => {
                if let Ok(mi) = child.clone().try_cast::<Node3D>() {
                    props.insert(
                        "visible".to_string(),
                        serde_json::Value::Bool(mi.is_visible()),
                    );
                }
            }
            "NavigationAgent3D" => {
                if let Ok(nav) = child.clone().try_cast::<godot::classes::NavigationAgent3D>() {
                    props.insert(
                        "target_reached".to_string(),
                        serde_json::Value::Bool(nav.is_target_reached()),
                    );
                    let dist = nav.distance_to_target();
                    if let Some(n) = serde_json::Number::from_f64(dist as f64) {
                        props.insert(
                            "distance_remaining".to_string(),
                            serde_json::Value::Number(n),
                        );
                    }
                }
            }
            "Area3D" | "Area2D" => {
                if let Ok(area) = child.clone().try_cast::<godot::classes::Area3D>() {
                    let bodies = area.get_overlapping_bodies();
                    let names: Vec<serde_json::Value> = (0..bodies.len())
                        .filter_map(|i| {
                            bodies.get(i).map(|b| {
                                serde_json::Value::String(b.get_name().to_string())
                            })
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

    fn collect_inspect_signals(&self, node: &Gd<Node>) -> InspectSignals {
        let mut connected = serde_json::Map::new();
        let signals: Array<VarDictionary> = node.get_signal_list();
        for i in 0..signals.len() {
            let Some(sig) = signals.get(i) else { continue };
            let name = sig
                .get(GString::from("name"))
                .and_then(|v| v.try_to::<GString>().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();
            if name.is_empty() { continue; }
            let conns = node.get_signal_connection_list(name.as_str());
            if conns.len() > 0 {
                let targets: Vec<serde_json::Value> = (0..conns.len())
                    .filter_map(|j| {
                        let conn = conns.get(j)?;
                        let callable = conn.get(GString::from("callable"))
                            .and_then(|v| v.try_to::<godot::builtin::Callable>().ok())?;
                        let obj_name = callable.object()
                            .map(|o| {
                                if let Ok(n) = o.try_cast::<Node>() {
                                    self.get_relative_path(&n)
                                } else {
                                    format!("<{}>", o.get_class())
                                }
                            })
                            .unwrap_or_else(|| "<unknown>".to_string());
                        let method = callable.method_name().to_string();
                        Some(serde_json::Value::String(format!("{obj_name}:{method}")))
                    })
                    .collect();
                connected.insert(name, serde_json::Value::Array(targets));
            }
        }

        InspectSignals {
            connected,
            recent_emissions: Vec::new(), // No emission tracking yet (requires M4 event system)
        }
    }

    fn collect_inspect_script(&self, node: &Gd<Node>) -> Option<InspectScript> {
        let script_variant = node.get_script()?;
        let script: Gd<Resource> = script_variant.upcast();
        let path = script.get_path().to_string();
        if path.is_empty() { return None; }

        // Get the base class from the script
        let base_class = node.get_class().to_string();

        // Get method list from the script
        let methods = if let Ok(gd_script) = script.clone().try_cast::<godot::classes::Script>() {
            let method_list: Array<VarDictionary> = gd_script.get_script_method_list();
            (0..method_list.len())
                .filter_map(|i| {
                    method_list.get(i).and_then(|m| {
                        m.get(GString::from("name"))
                            .and_then(|v| v.try_to::<GString>().ok())
                            .map(|s| s.to_string())
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        // Build extends chain
        let mut extends_chain = Vec::new();
        let mut current_class = base_class.clone();
        extends_chain.push(current_class.clone());
        // Walk the Godot class hierarchy
        let class_db = godot::classes::ClassDb::singleton();
        loop {
            let parent = class_db
                .get_parent_class(current_class.as_str())
                .to_string();
            if parent.is_empty() || parent == current_class {
                break;
            }
            extends_chain.push(parent.clone());
            current_class = parent;
        }

        Some(InspectScript {
            path,
            base_class,
            methods,
            extends_chain,
        })
    }

    /// Collect raw spatial context data for a node.
    /// Server will post-process with bearing calculations.
    fn collect_spatial_context_raw(&self, node: &Gd<Node>) -> SpatialContextRaw {
        let node3d = match node.clone().try_cast::<Node3D>() {
            Ok(n) => n,
            Err(_) => return SpatialContextRaw {
                nearby: Vec::new(),
                in_areas: Vec::new(),
                camera_visible: false,
                camera_distance: 0.0,
                node_position: Vec::new(),
                node_forward: Vec::new(),
            },
        };

        let pos = node3d.get_global_position();
        let fwd_col = node3d.get_global_transform().basis.col_c();
        let node_position = vec![pos.x as f64, pos.y as f64, pos.z as f64];
        let node_forward = vec![-fwd_col.x as f64, -fwd_col.y as f64, -fwd_col.z as f64];

        // Camera info
        let (camera_visible, camera_distance) = if let Some(vp) = self.base().get_viewport() {
            if let Some(camera) = vp.get_camera_3d() {
                let cam_pos = camera.get_global_position();
                let dist = pos.distance_to(cam_pos) as f64;
                let visible = node3d.is_visible_in_tree(); // simplified visibility check
                (visible, dist)
            } else {
                (false, 0.0)
            }
        } else {
            (false, 0.0)
        };

        // Nearby entities: collect up to 10 nearest Node3D siblings/cousins
        let mut nearby = Vec::new();
        if let Some(tree) = self.base().get_tree() {
            if let Some(root) = tree.get_current_scene() {
                self.collect_nearby_recursive(&root, &pos, &node, &mut nearby, 20.0);
                // Sort by distance, keep nearest 10
                nearby.sort_by(|a, b| {
                    let da = position_distance(&a.position, &node_position);
                    let db = position_distance(&b.position, &node_position);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
                nearby.truncate(10);
            }
        }

        // Areas the node is in
        let in_areas = self.collect_containing_areas(&node3d);

        SpatialContextRaw {
            nearby,
            in_areas,
            camera_visible,
            camera_distance,
            node_position,
            node_forward,
        }
    }

    /// Recursively find nearby Node3D nodes within radius.
    fn collect_nearby_recursive(
        &self,
        node: &Gd<Node>,
        target_pos: &Vector3,
        exclude: &Gd<Node>,
        result: &mut Vec<NearbyEntityRaw>,
        radius: f64,
    ) {
        if self.is_spectator_node(node) { return; }
        if node.instance_id() == exclude.instance_id() { return; }

        if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
            let pos = n3d.get_global_position();
            let dist = pos.distance_to(*target_pos) as f64;
            if dist <= radius {
                let node_ref: Gd<Node> = n3d.clone().upcast();
                result.push(NearbyEntityRaw {
                    path: self.get_relative_path(&node_ref),
                    class: node_ref.get_class().to_string(),
                    position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
                    groups: self.get_groups(&node_ref),
                });
            }
        }

        let count = node.get_child_count();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                self.collect_nearby_recursive(&child, target_pos, exclude, result, radius);
            }
        }
    }

    /// Find Area3D nodes that contain (overlap) the target node.
    fn collect_containing_areas(&self, node: &Gd<Node3D>) -> Vec<String> {
        let mut areas = Vec::new();
        if let Some(tree) = self.base().get_tree() {
            // Get all Area3D nodes in the scene
            let area_nodes = tree.get_nodes_in_group("".into()); // we'll iterate the tree instead
            if let Some(root) = tree.get_current_scene() {
                self.find_areas_containing(&root, node, &mut areas);
            }
        }
        areas
    }

    fn find_areas_containing(
        &self,
        node: &Gd<Node>,
        target: &Gd<Node3D>,
        result: &mut Vec<String>,
    ) {
        if let Ok(area) = node.clone().try_cast::<godot::classes::Area3D>() {
            let bodies = area.get_overlapping_bodies();
            for i in 0..bodies.len() {
                if let Some(body) = bodies.get(i) {
                    if body.instance_id() == target.instance_id() {
                        let area_node: Gd<Node> = area.clone().upcast();
                        result.push(self.get_relative_path(&area_node));
                        break;
                    }
                }
            }
        }

        let count = node.get_child_count();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                self.find_areas_containing(&child, target, result);
            }
        }
    }
}

/// Euclidean distance between two position arrays.
fn position_distance(a: &[f64], b: &[f64]) -> f64 {
    let dx = a.first().unwrap_or(&0.0) - b.first().unwrap_or(&0.0);
    let dy = a.get(1).unwrap_or(&0.0) - b.get(1).unwrap_or(&0.0);
    let dz = a.get(2).unwrap_or(&0.0) - b.get(2).unwrap_or(&0.0);
    (dx * dx + dy * dy + dz * dz).sqrt()
}
```

**Implementation Notes:**
- `collect_inspect_signals` retrieves actual signal connection targets by iterating the connection list and extracting Callable info. This is more detailed than `get_connected_signals()` from M1.
- `collect_inspect_script` uses `ClassDb` to build the extends chain. `Script::get_script_method_list()` returns only script-defined methods, not inherited engine methods.
- `collect_child_props` is selective — only populates props for child types where summary info is useful (collision shapes, nav agents, areas). Other children get empty props.
- `collect_spatial_context_raw` does a full tree walk to find nearby entities. This is O(n) over the scene tree. For M2, this is acceptable — spatial indexing (rstar) is deferred to M3. The radius is capped at 20 units to limit output.
- `CharacterBody3D.is_on_wall()` and `is_on_ceiling()` are added to physics data (were missing from M1).

**Acceptance Criteria:**
- [ ] `inspect_node` returns correct data for all 7 include categories
- [ ] Missing categories are `None` in response (not empty defaults)
- [ ] Non-3D nodes return empty transform gracefully
- [ ] Non-physics nodes return `None` for physics
- [ ] Signal connection targets resolve to relative node paths
- [ ] Script methods list populates from Script API
- [ ] Extends chain walks the full class hierarchy
- [ ] Spatial context returns nearest 10 entities within 20 units
- [ ] Spectator internal nodes are excluded from nearby entities

---

### Unit 3: GDExtension Collector — Scene Tree Queries

**File:** `crates/spectator-godot/src/collector.rs`

Add scene tree navigation methods to `SpectatorCollector`.

```rust
impl SpectatorCollector {
    /// Handle scene tree queries.
    pub fn query_scene_tree(
        &self,
        params: &GetSceneTreeParams,
    ) -> Result<serde_json::Value, String> {
        match params.action {
            SceneTreeAction::Roots => self.scene_tree_roots(&params.include),
            SceneTreeAction::Children => {
                let path = params.node.as_ref()
                    .ok_or("'node' is required for 'children' action")?;
                self.scene_tree_children(path, &params.include)
            }
            SceneTreeAction::Subtree => {
                let path = params.node.as_ref()
                    .ok_or("'node' is required for 'subtree' action")?;
                self.scene_tree_subtree(path, params.depth, &params.include)
            }
            SceneTreeAction::Ancestors => {
                let path = params.node.as_ref()
                    .ok_or("'node' is required for 'ancestors' action")?;
                self.scene_tree_ancestors(path, &params.include)
            }
            SceneTreeAction::Find => {
                let find_by = params.find_by.as_ref()
                    .ok_or("'find_by' is required for 'find' action")?;
                let find_value = params.find_value.as_ref()
                    .ok_or("'find_value' is required for 'find' action")?;
                self.scene_tree_find(*find_by, find_value, &params.include)
            }
        }
    }

    fn scene_tree_roots(
        &self,
        include: &[TreeInclude],
    ) -> Result<serde_json::Value, String> {
        let tree = self.base().get_tree()
            .ok_or("No scene tree")?;
        let root = tree.get_root()
            .ok_or("No root node")?;

        let mut roots = Vec::new();
        let count = root.get_child_count();
        for i in 0..count {
            if let Some(child) = root.get_child(i) {
                if self.is_spectator_node(&child) { continue; }
                roots.push(self.node_info(&child, include));
            }
        }

        Ok(serde_json::json!({ "roots": roots }))
    }

    fn scene_tree_children(
        &self,
        path: &str,
        include: &[TreeInclude],
    ) -> Result<serde_json::Value, String> {
        let node = self.resolve_node(path)?;
        let count = node.get_child_count();
        let mut children = Vec::new();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                if self.is_spectator_node(&child) { continue; }
                children.push(self.node_info(&child, include));
            }
        }
        Ok(serde_json::json!({
            "node": path,
            "children": children,
        }))
    }

    fn scene_tree_subtree(
        &self,
        path: &str,
        max_depth: u32,
        include: &[TreeInclude],
    ) -> Result<serde_json::Value, String> {
        let node = self.resolve_node(path)?;
        let tree = self.build_subtree(&node, max_depth, 0, include);
        let total = self.count_nodes(&node);
        Ok(serde_json::json!({
            "root": path,
            "tree": tree,
            "total_nodes": total,
            "depth_reached": max_depth,
        }))
    }

    fn build_subtree(
        &self,
        node: &Gd<Node>,
        max_depth: u32,
        current_depth: u32,
        include: &[TreeInclude],
    ) -> serde_json::Value {
        let mut info = self.node_info(node, include);

        if current_depth < max_depth {
            let count = node.get_child_count();
            let mut children = serde_json::Map::new();
            for i in 0..count {
                if let Some(child) = node.get_child(i) {
                    if self.is_spectator_node(&child) { continue; }
                    let name = child.get_name().to_string();
                    let child_tree = self.build_subtree(
                        &child,
                        max_depth,
                        current_depth + 1,
                        include,
                    );
                    children.insert(name, child_tree);
                }
            }
            if !children.is_empty() {
                if let serde_json::Value::Object(ref mut map) = info {
                    map.insert("children".to_string(),
                        serde_json::Value::Object(children));
                }
            }
        } else {
            // At depth limit, check if there are children
            if node.get_child_count() > 0 {
                if let serde_json::Value::Object(ref mut map) = info {
                    map.insert("children".to_string(),
                        serde_json::json!({"...": "depth_limit_reached"}));
                }
            }
        }

        info
    }

    fn scene_tree_ancestors(
        &self,
        path: &str,
        include: &[TreeInclude],
    ) -> Result<serde_json::Value, String> {
        let node = self.resolve_node(path)?;
        let mut ancestors = Vec::new();

        // Start with the node itself
        ancestors.push(self.node_info(&node, include));

        // Walk up to root
        let mut current: Gd<Node> = node;
        while let Some(parent) = current.get_parent() {
            // Stop at the viewport root
            if parent.get_parent().is_none() { break; }
            ancestors.push(self.node_info(&parent, include));
            current = parent;
        }

        Ok(serde_json::json!({
            "node": path,
            "ancestors": ancestors,
        }))
    }

    fn scene_tree_find(
        &self,
        find_by: FindBy,
        value: &str,
        include: &[TreeInclude],
    ) -> Result<serde_json::Value, String> {
        let tree = self.base().get_tree()
            .ok_or("No scene tree")?;
        let root = tree.get_current_scene()
            .ok_or("No current scene")?;

        let mut results = Vec::new();
        self.find_recursive(&root, find_by, value, include, &mut results);

        Ok(serde_json::json!({
            "find_by": find_by,
            "find_value": value,
            "results": results,
        }))
    }

    fn find_recursive(
        &self,
        node: &Gd<Node>,
        find_by: FindBy,
        value: &str,
        include: &[TreeInclude],
        results: &mut Vec<serde_json::Value>,
    ) {
        if self.is_spectator_node(node) { return; }

        let matches = match find_by {
            FindBy::Name => node.get_name().to_string().contains(value),
            FindBy::Class => node.get_class().to_string() == value,
            FindBy::Group => node.is_in_group(value),
            FindBy::Script => {
                self.get_script_path(node)
                    .as_deref()
                    .map(|p| p == value)
                    .unwrap_or(false)
            }
        };

        if matches {
            let mut info = self.node_info(node, include);
            if let serde_json::Value::Object(ref mut map) = info {
                map.insert("path".to_string(),
                    serde_json::Value::String(self.get_relative_path(node)));
            }
            results.push(info);
        }

        let count = node.get_child_count();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                self.find_recursive(&child, find_by, value, include, results);
            }
        }
    }

    /// Build a node info object with requested includes.
    fn node_info(
        &self,
        node: &Gd<Node>,
        include: &[TreeInclude],
    ) -> serde_json::Value {
        let mut info = serde_json::Map::new();
        let name = node.get_name().to_string();
        info.insert("name".to_string(), serde_json::json!(name));

        for inc in include {
            match inc {
                TreeInclude::Class => {
                    info.insert("class".to_string(),
                        serde_json::json!(node.get_class().to_string()));
                }
                TreeInclude::Groups => {
                    let groups = self.get_groups(node);
                    if !groups.is_empty() {
                        info.insert("groups".to_string(), serde_json::json!(groups));
                    }
                }
                TreeInclude::Script => {
                    if let Some(path) = self.get_script_path(node) {
                        info.insert("script".to_string(), serde_json::json!(path));
                    }
                }
                TreeInclude::Visible => {
                    if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
                        info.insert("visible".to_string(),
                            serde_json::json!(n3d.is_visible_in_tree()));
                    }
                }
                TreeInclude::ProcessMode => {
                    let mode = node.get_process_mode();
                    info.insert("process_mode".to_string(),
                        serde_json::json!(format!("{:?}", mode)));
                }
            }
        }

        serde_json::Value::Object(info)
    }

    /// Resolve a node path to a Gd<Node>.
    fn resolve_node(&self, path: &str) -> Result<Gd<Node>, String> {
        let tree = self.base().get_tree()
            .ok_or("No scene tree")?;
        let root = tree.get_current_scene()
            .ok_or("No current scene")?;
        root.try_get_node_as::<Node>(path)
            .ok_or_else(|| format!("Node '{}' not found", path))
    }

    /// Count total nodes in a subtree.
    fn count_nodes(&self, node: &Gd<Node>) -> usize {
        let mut count = 1;
        let child_count = node.get_child_count();
        for i in 0..child_count {
            if let Some(child) = node.get_child(i) {
                count += self.count_nodes(&child);
            }
        }
        count
    }
}
```

**Implementation Notes:**
- `scene_tree_roots` returns children of the viewport root (scene root + autoloads), filtering Spectator internals.
- `build_subtree` produces the nested object format from CONTRACT.md: `{ "name": ..., "class": ..., "children": { "child_name": { ... } } }`.
- `scene_tree_ancestors` includes the target node first, then walks up to the scene root.
- `find_recursive` uses substring match for `Name` (lenient) but exact match for `Class` (Godot classes are precise). `Group` uses `is_in_group`, `Script` uses exact path match.
- `node_info` is a shared helper that builds per-node JSON based on `TreeInclude` options.

**Acceptance Criteria:**
- [ ] `roots` returns scene root and autoloads (excluding Spectator nodes)
- [ ] `children` returns immediate children of a specified node
- [ ] `subtree` respects max depth with `"...": "depth_limit_reached"` sentinel
- [ ] `ancestors` returns chain from node to scene root
- [ ] `find` by name uses substring match
- [ ] `find` by class uses exact match
- [ ] `find` by group uses `is_in_group`
- [ ] `find` by script uses exact path match
- [ ] All results respect `include` configuration
- [ ] `node_not_found` error for invalid paths

---

### Unit 4: Query Handler — Routing New Methods

**File:** `crates/spectator-godot/src/query_handler.rs`

Extend the query handler to dispatch the two new query methods.

```rust
use spectator_protocol::{
    messages::Message,
    query::{GetNodeInspectParams, GetSceneTreeParams, GetSnapshotDataParams},
};

use crate::collector::SpectatorCollector;

pub fn handle_query(
    id: String,
    method: &str,
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Message {
    let result = match method {
        "get_snapshot_data" => handle_get_snapshot_data(params, collector),
        "get_frame_info" => handle_get_frame_info(collector),
        "get_node_inspect" => handle_get_node_inspect(params, collector),
        "get_scene_tree" => handle_get_scene_tree(params, collector),
        _ => Err(QueryError {
            code: "method_not_found".to_string(),
            message: format!("Unknown query method: {method}"),
        }),
    };

    match result {
        Ok(data) => Message::Response { id, data },
        Err(e) => Message::Error {
            id,
            code: e.code,
            message: e.message,
        },
    }
}

// ... existing handle_get_snapshot_data and handle_get_frame_info ...

fn handle_get_node_inspect(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetNodeInspectParams = serde_json::from_value(params)
        .map_err(|e| QueryError {
            code: "invalid_params".to_string(),
            message: format!("Invalid params: {e}"),
        })?;

    let data = collector.inspect_node(&params)
        .map_err(|e| QueryError {
            code: "node_not_found".to_string(),
            message: e,
        })?;

    serde_json::to_value(&data).map_err(|e| QueryError {
        code: "internal".to_string(),
        message: format!("Serialization error: {e}"),
    })
}

fn handle_get_scene_tree(
    params: serde_json::Value,
    collector: &SpectatorCollector,
) -> Result<serde_json::Value, QueryError> {
    let params: GetSceneTreeParams = serde_json::from_value(params)
        .map_err(|e| QueryError {
            code: "invalid_params".to_string(),
            message: format!("Invalid params: {e}"),
        })?;

    let data = collector.query_scene_tree(&params)
        .map_err(|e| QueryError {
            code: "node_not_found".to_string(),
            message: e,
        })?;

    Ok(data)
}
```

**Acceptance Criteria:**
- [ ] `get_node_inspect` dispatches to `collector.inspect_node()`
- [ ] `get_scene_tree` dispatches to `collector.query_scene_tree()`
- [ ] Invalid params return `invalid_params` error code
- [ ] Node-not-found errors return `node_not_found` error code

---

### Unit 5: MCP Tool — `spatial_inspect`

**File:** `crates/spectator-server/src/mcp/inspect.rs` (new file)

```rust
use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use spectator_core::{bearing, types::Position3};
use spectator_protocol::query::{
    GetNodeInspectParams, InspectCategory, NodeInspectResponse, NearbyEntityRaw,
};

use crate::tcp::query_addon;

/// Parameters for the spatial_inspect MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialInspectParams {
    /// Node path to inspect (relative to scene root).
    pub node: String,

    /// Which data categories to include.
    /// Options: "transform", "physics", "state", "children", "signals",
    ///          "script", "spatial_context"
    /// Default: all categories.
    #[serde(default = "default_include")]
    pub include: Vec<String>,
}

fn default_include() -> Vec<String> {
    vec![
        "transform".into(), "physics".into(), "state".into(),
        "children".into(), "signals".into(), "script".into(),
        "spatial_context".into(),
    ]
}

/// Parse include strings to InspectCategory enums.
pub fn parse_include(strings: &[String]) -> Result<Vec<InspectCategory>, McpError> {
    strings
        .iter()
        .map(|s| match s.as_str() {
            "transform" => Ok(InspectCategory::Transform),
            "physics" => Ok(InspectCategory::Physics),
            "state" => Ok(InspectCategory::State),
            "children" => Ok(InspectCategory::Children),
            "signals" => Ok(InspectCategory::Signals),
            "script" => Ok(InspectCategory::Script),
            "spatial_context" => Ok(InspectCategory::SpatialContext),
            other => Err(McpError::invalid_params(
                format!("Invalid include category '{other}'. Options: transform, physics, state, children, signals, script, spatial_context"),
                None,
            )),
        })
        .collect()
}

/// Build the spatial_context block from raw addon data.
/// Computes bearings server-side from the raw positions.
pub fn build_spatial_context(
    raw: &spectator_protocol::query::SpatialContextRaw,
) -> serde_json::Value {
    let node_pos: Position3 = [
        raw.node_position.first().copied().unwrap_or(0.0),
        raw.node_position.get(1).copied().unwrap_or(0.0),
        raw.node_position.get(2).copied().unwrap_or(0.0),
    ];
    let node_fwd = [
        raw.node_forward.first().copied().unwrap_or(0.0),
        raw.node_forward.get(1).copied().unwrap_or(0.0),
        raw.node_forward.get(2).copied().unwrap_or(-1.0),
    ];

    let perspective = bearing::perspective_from_forward(node_pos, node_fwd);

    let nearby_entities: Vec<serde_json::Value> = raw
        .nearby
        .iter()
        .map(|e| {
            let target_pos: Position3 = [
                e.position.first().copied().unwrap_or(0.0),
                e.position.get(1).copied().unwrap_or(0.0),
                e.position.get(2).copied().unwrap_or(0.0),
            ];
            let rel = bearing::relative_position(&perspective, target_pos, false);
            let mut entry = serde_json::json!({
                "path": e.path,
                "dist": rel.dist,
                "bearing": rel.bearing,
                "class": e.class,
            });
            if !e.groups.is_empty() {
                entry["group"] = serde_json::json!(e.groups.first().unwrap_or(&String::new()));
            }
            entry
        })
        .collect();

    serde_json::json!({
        "nearby_entities": nearby_entities,
        "in_areas": raw.in_areas,
        "camera_visible": raw.camera_visible,
        "camera_distance": raw.camera_distance,
    })
}
```

**File:** `crates/spectator-core/src/bearing.rs`

Add one new function:

```rust
/// Create a Perspective from a position and explicit forward vector.
pub fn perspective_from_forward(position: Position3, forward: [f64; 3]) -> Perspective {
    let (facing, facing_deg) = compass_bearing(forward);
    Perspective {
        position,
        forward,
        facing,
        facing_deg,
    }
}
```

**Implementation Notes:**
- `spatial_context` bearings are computed relative to the *inspected node's* facing, not the camera. This answers "what's ahead of/behind this enemy?"
- `perspective_from_forward` is a new helper in `bearing.rs` that creates a `Perspective` from a forward vector instead of a yaw angle. The existing `perspective_from_yaw` computes the forward from yaw; this one takes the forward directly.
- The response is formatted to match CONTRACT.md's `spatial_inspect` shape.

**Acceptance Criteria:**
- [ ] `parse_include` validates all 7 category strings
- [ ] Invalid category returns descriptive error
- [ ] `build_spatial_context` computes bearings relative to the inspected node
- [ ] Nearby entities include path, dist, bearing, class, and first group
- [ ] `perspective_from_forward` produces correct bearing results

---

### Unit 6: MCP Tool — `scene_tree`

**File:** `crates/spectator-server/src/mcp/scene_tree.rs` (new file)

```rust
use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use spectator_core::budget::{BudgetEnforcer, SnapshotBudgetDefaults, resolve_budget};
use spectator_protocol::query::{
    FindBy, GetSceneTreeParams, SceneTreeAction, TreeInclude,
};

use crate::tcp::query_addon;

/// Parameters for the scene_tree MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SceneTreeToolParams {
    /// Action: "roots", "children", "subtree", "ancestors", "find"
    pub action: String,

    /// Node path — required for children, subtree, ancestors.
    pub node: Option<String>,

    /// Max recursion depth for subtree. Default: 3.
    #[serde(default = "default_depth")]
    pub depth: Option<u32>,

    /// For find: search by "name", "class", "group", or "script".
    pub find_by: Option<String>,

    /// For find: search value.
    pub find_value: Option<String>,

    /// What to include per node: "class", "groups", "script", "visible", "process_mode".
    /// Default: ["class", "groups"].
    #[serde(default = "default_include")]
    pub include: Option<Vec<String>>,

    /// Soft token budget override.
    pub token_budget: Option<u32>,
}

fn default_depth() -> Option<u32> {
    Some(3)
}
fn default_include() -> Option<Vec<String>> {
    Some(vec!["class".into(), "groups".into()])
}

pub fn parse_action(s: &str) -> Result<SceneTreeAction, McpError> {
    match s {
        "roots" => Ok(SceneTreeAction::Roots),
        "children" => Ok(SceneTreeAction::Children),
        "subtree" => Ok(SceneTreeAction::Subtree),
        "ancestors" => Ok(SceneTreeAction::Ancestors),
        "find" => Ok(SceneTreeAction::Find),
        other => Err(McpError::invalid_params(
            format!("Invalid action '{other}'. Must be 'roots', 'children', 'subtree', 'ancestors', or 'find'."),
            None,
        )),
    }
}

pub fn parse_find_by(s: &str) -> Result<FindBy, McpError> {
    match s {
        "name" => Ok(FindBy::Name),
        "class" => Ok(FindBy::Class),
        "group" => Ok(FindBy::Group),
        "script" => Ok(FindBy::Script),
        other => Err(McpError::invalid_params(
            format!("Invalid find_by '{other}'. Must be 'name', 'class', 'group', or 'script'."),
            None,
        )),
    }
}

pub fn parse_tree_include(strings: &[String]) -> Result<Vec<TreeInclude>, McpError> {
    strings
        .iter()
        .map(|s| match s.as_str() {
            "class" => Ok(TreeInclude::Class),
            "groups" => Ok(TreeInclude::Groups),
            "script" => Ok(TreeInclude::Script),
            "visible" => Ok(TreeInclude::Visible),
            "process_mode" => Ok(TreeInclude::ProcessMode),
            other => Err(McpError::invalid_params(
                format!("Invalid include '{other}'. Options: class, groups, script, visible, process_mode"),
                None,
            )),
        })
        .collect()
}

/// Build the GetSceneTreeParams from MCP tool params.
pub fn build_scene_tree_params(
    params: &SceneTreeToolParams,
) -> Result<GetSceneTreeParams, McpError> {
    let action = parse_action(&params.action)?;
    let include_strs = params.include.as_deref()
        .unwrap_or(&["class".to_string(), "groups".to_string()][..]);
    // Can't use borrowed slice of temporary, so handle differently:
    let default_inc = vec!["class".to_string(), "groups".to_string()];
    let include_strs = params.include.as_deref().unwrap_or(&default_inc);
    let include = parse_tree_include(include_strs)?;

    let find_by = params.find_by.as_deref()
        .map(parse_find_by)
        .transpose()?;

    Ok(GetSceneTreeParams {
        action,
        node: params.node.clone(),
        depth: params.depth.unwrap_or(3),
        find_by,
        find_value: params.find_value.clone(),
        include,
    })
}
```

**Acceptance Criteria:**
- [ ] All 5 actions parse correctly
- [ ] Invalid action/find_by/include returns descriptive error
- [ ] `build_scene_tree_params` translates MCP strings to protocol enums

---

### Unit 7: MCP Tool Router — Register Both Tools

**File:** `crates/spectator-server/src/mcp/mod.rs`

Update to add both new tools alongside `spatial_snapshot`.

```rust
pub mod inspect;
pub mod scene_tree;
pub mod snapshot;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use rmcp::tool_router;
use spectator_core::{bearing, budget::SnapshotBudgetDefaults, budget::resolve_budget, types::Position3};
use spectator_protocol::query::{
    DetailLevel, GetNodeInspectParams, GetSceneTreeParams, GetSnapshotDataParams,
    NodeInspectResponse, SnapshotResponse,
};

use crate::server::SpectatorServer;
use crate::tcp::query_addon;
use inspect::{SpatialInspectParams, build_spatial_context, parse_include};
use scene_tree::{SceneTreeToolParams, build_scene_tree_params};
use snapshot::{
    SpatialSnapshotParams, build_expand_response, build_full_response, build_perspective,
    build_perspective_param, build_standard_response, build_summary_response, parse_detail,
};

#[tool_router(vis = "pub")]
impl SpectatorServer {
    /// Get a spatial snapshot of the current scene from a perspective.
    #[tool(description = "Get a spatial snapshot of the current scene from a perspective. Use detail 'summary' for a cheap overview (~200 tokens), 'standard' for per-entity data (~400-800 tokens), or 'full' for everything including transforms, physics, and children (~1000+ tokens). Start with summary, then drill down.")]
    pub async fn spatial_snapshot(
        &self,
        Parameters(params): Parameters<SpatialSnapshotParams>,
    ) -> Result<String, McpError> {
        // ... existing implementation unchanged ...
    }

    /// Deep inspection of a single node — transform, physics, state, children,
    /// signals, script, and spatial context. The "tell me everything about this
    /// one thing" tool.
    #[tool(description = "Deep inspection of a single node. Returns transform, physics, state, children, signals, script, and spatial context. Use the 'include' parameter to select specific categories and reduce token usage. Default includes all categories.")]
    pub async fn spatial_inspect(
        &self,
        Parameters(params): Parameters<SpatialInspectParams>,
    ) -> Result<String, McpError> {
        let include = parse_include(&params.include)?;

        let query_params = GetNodeInspectParams {
            path: params.node.clone(),
            include: include.clone(),
        };

        let raw_data: NodeInspectResponse = {
            let data = query_addon(
                &self.state,
                "get_node_inspect",
                serde_json::to_value(&query_params).map_err(|e| {
                    McpError::internal_error(format!("Param serialization error: {e}"), None)
                })?,
            )
            .await?;
            serde_json::from_value(data).map_err(|e| {
                McpError::internal_error(format!("Response deserialization error: {e}"), None)
            })?
        };

        // Post-process spatial_context with bearing calculations
        let mut response = serde_json::to_value(&raw_data).map_err(|e| {
            McpError::internal_error(format!("Serialization error: {e}"), None)
        })?;

        if let Some(raw_ctx) = &raw_data.spatial_context_raw {
            let spatial_context = build_spatial_context(raw_ctx);
            if let serde_json::Value::Object(ref mut map) = response {
                map.remove("spatial_context_raw");
                map.insert("spatial_context".to_string(), spatial_context);
            }
        }

        // Add budget
        let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
        let used = spectator_core::budget::estimate_tokens(json_bytes);
        if let serde_json::Value::Object(ref mut map) = response {
            map.insert("budget".to_string(), serde_json::json!({
                "used": used,
                "limit": 1500,
                "hard_cap": SnapshotBudgetDefaults::HARD_CAP,
            }));
        }

        serde_json::to_string(&response).map_err(|e| {
            McpError::internal_error(format!("Response serialization error: {e}"), None)
        })
    }

    /// Navigate and query the Godot scene tree structure. Not spatial — this is
    /// about understanding the node hierarchy.
    #[tool(description = "Navigate the Godot scene tree. Actions: 'roots' (top-level nodes), 'children' (immediate children), 'subtree' (recursive tree with depth limit), 'ancestors' (parent chain to root), 'find' (search by name/class/group/script). Use 'include' to control per-node data.")]
    pub async fn scene_tree(
        &self,
        Parameters(params): Parameters<SceneTreeToolParams>,
    ) -> Result<String, McpError> {
        let query_params = build_scene_tree_params(&params)?;

        let data = query_addon(
            &self.state,
            "get_scene_tree",
            serde_json::to_value(&query_params).map_err(|e| {
                McpError::internal_error(format!("Param serialization error: {e}"), None)
            })?,
        )
        .await?;

        // Add budget
        let json_bytes = serde_json::to_vec(&data).unwrap_or_default().len();
        let used = spectator_core::budget::estimate_tokens(json_bytes);
        let budget_limit = resolve_budget(
            params.token_budget,
            1500, // scene_tree default
            SnapshotBudgetDefaults::HARD_CAP,
        );

        let mut response = data;
        if let serde_json::Value::Object(ref mut map) = response {
            map.insert("budget".to_string(), serde_json::json!({
                "used": used,
                "limit": budget_limit,
                "hard_cap": SnapshotBudgetDefaults::HARD_CAP,
            }));
        }

        serde_json::to_string(&response).map_err(|e| {
            McpError::internal_error(format!("Response serialization error: {e}"), None)
        })
    }
}
```

**Implementation Notes:**
- `spatial_inspect` queries the addon for raw data, then post-processes `spatial_context_raw` by computing bearings server-side. The raw field is removed and replaced with the processed `spatial_context` field in the output.
- `scene_tree` is a pass-through to the addon for most actions. The server adds the `budget` block. Token budget enforcement (truncation) is not needed for scene_tree in M2 — the depth parameter limits output naturally.
- Both tools reuse the existing `query_addon` function from `tcp.rs`.

**Acceptance Criteria:**
- [ ] `spatial_inspect` appears in MCP tool listing with description
- [ ] `scene_tree` appears in MCP tool listing with description
- [ ] `spatial_inspect` returns processed spatial_context (not raw)
- [ ] `spatial_inspect` includes budget block
- [ ] `scene_tree` includes budget block
- [ ] Both tools return `not_connected` when addon is unavailable
- [ ] Both tools return `node_not_found` for invalid paths
- [ ] `spatial_inspect` defaults to all 7 categories when `include` is omitted
- [ ] `scene_tree` defaults to `["class", "groups"]` for include

---

## Implementation Order

1. **Unit 1: Protocol types** — Foundation for everything else
2. **Unit 5 (partial): `bearing::perspective_from_forward`** — Small helper needed by inspect
3. **Unit 2: GDExtension collector — inspect** — Addon must answer inspect queries
4. **Unit 3: GDExtension collector — scene tree** — Addon must answer scene tree queries
5. **Unit 4: Query handler routing** — Wire the new methods into the dispatcher
6. **Unit 5 (remaining): MCP inspect module** — Server-side processing for inspect
7. **Unit 6: MCP scene_tree module** — Server-side processing for scene_tree
8. **Unit 7: Tool router** — Register both tools with rmcp

Units 2 and 3 can be done in parallel once Unit 1 is complete. Units 5 and 6 can be done in parallel once Units 2-4 are complete.

---

## Testing

### Unit Tests: `crates/spectator-protocol/src/query.rs`

Add to existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn inspect_params_round_trip() {
    let params = GetNodeInspectParams {
        path: "enemies/scout_02".to_string(),
        include: vec![InspectCategory::Transform, InspectCategory::Physics],
    };
    let json = serde_json::to_string(&params).unwrap();
    let parsed: GetNodeInspectParams = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.path, "enemies/scout_02");
    assert_eq!(parsed.include.len(), 2);
}

#[test]
fn inspect_category_rename() {
    let cat = InspectCategory::SpatialContext;
    let json = serde_json::to_string(&cat).unwrap();
    assert_eq!(json, r#""spatial_context""#);
}

#[test]
fn scene_tree_params_round_trip() {
    let params = GetSceneTreeParams {
        action: SceneTreeAction::Find,
        node: None,
        depth: 3,
        find_by: Some(FindBy::Class),
        find_value: Some("CharacterBody3D".into()),
        include: vec![TreeInclude::Class, TreeInclude::Groups],
    };
    let json = serde_json::to_string(&params).unwrap();
    let parsed: GetSceneTreeParams = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.find_value, Some("CharacterBody3D".into()));
}

#[test]
fn scene_tree_action_rename() {
    let action = SceneTreeAction::Subtree;
    let json = serde_json::to_string(&action).unwrap();
    assert_eq!(json, r#""subtree""#);
}

#[test]
fn inspect_response_optional_fields() {
    let response = NodeInspectResponse {
        path: "test".into(),
        class: "Node3D".into(),
        instance_id: 12345,
        transform: None,
        physics: None,
        state: None,
        children: None,
        signals: None,
        script: None,
        spatial_context_raw: None,
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(!json.contains("transform"));
    assert!(!json.contains("physics"));
}
```

### Unit Tests: `crates/spectator-core/src/bearing.rs`

```rust
#[test]
fn perspective_from_forward_negative_z() {
    let p = perspective_from_forward([0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
    assert_eq!(p.facing, Cardinal::Ahead);
    assert!(p.facing_deg.abs() < 1.0);
}
```

### Unit Tests: `crates/spectator-server/src/mcp/inspect.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_include_valid() {
        let include = vec!["transform".into(), "physics".into()];
        let result = parse_include(&include).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], InspectCategory::Transform);
    }

    #[test]
    fn parse_include_invalid() {
        let include = vec!["invalid".into()];
        assert!(parse_include(&include).is_err());
    }

    #[test]
    fn build_spatial_context_computes_bearing() {
        let raw = spectator_protocol::query::SpatialContextRaw {
            nearby: vec![NearbyEntityRaw {
                path: "enemy".into(),
                class: "CharacterBody3D".into(),
                position: vec![0.0, 0.0, -10.0],
                groups: vec!["enemies".into()],
            }],
            in_areas: vec!["zone_a".into()],
            camera_visible: true,
            camera_distance: 15.0,
            node_position: vec![0.0, 0.0, 0.0],
            node_forward: vec![0.0, 0.0, -1.0],
        };
        let ctx = build_spatial_context(&raw);
        let nearby = ctx["nearby_entities"].as_array().unwrap();
        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0]["bearing"], "ahead");
    }
}
```

### Unit Tests: `crates/spectator-server/src/mcp/scene_tree.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_action_valid() {
        assert_eq!(parse_action("roots").unwrap(), SceneTreeAction::Roots);
        assert_eq!(parse_action("find").unwrap(), SceneTreeAction::Find);
    }

    #[test]
    fn parse_action_invalid() {
        assert!(parse_action("invalid").is_err());
    }

    #[test]
    fn parse_find_by_valid() {
        assert_eq!(parse_find_by("class").unwrap(), FindBy::Class);
        assert_eq!(parse_find_by("group").unwrap(), FindBy::Group);
    }

    #[test]
    fn parse_tree_include_valid() {
        let inc = vec!["class".into(), "script".into()];
        let result = parse_tree_include(&inc).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_tree_include_invalid() {
        let inc = vec!["invalid".into()];
        assert!(parse_tree_include(&inc).is_err());
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

# Manual verification (with running Godot game):
# 1. spatial_inspect(node: "enemies/scout_02")
#    → verify all 7 categories return data
# 2. spatial_inspect(node: "enemies/scout_02", include: ["physics", "state"])
#    → verify only physics and state are present
# 3. spatial_inspect(node: "nonexistent")
#    → verify node_not_found error
# 4. scene_tree(action: "roots")
#    → verify scene root and autoloads listed
# 5. scene_tree(action: "subtree", node: "enemies", depth: 2)
#    → verify nested tree structure
# 6. scene_tree(action: "find", find_by: "class", find_value: "CharacterBody3D")
#    → verify matching nodes returned
# 7. scene_tree(action: "ancestors", node: "enemies/scout_02/NavAgent")
#    → verify parent chain
# 8. Both tools return not_connected when game not running
```
