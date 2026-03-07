use godot::builtin::{Array, GString, StringName, Variant, VarDictionary, Vector2, Vector3};
use godot::builtin::VariantType;
use godot::classes::{
    CharacterBody3D, Engine, NavigationServer3D, Node, Node2D, Node3D, PhysicsBody3D,
    PhysicsRayQueryParameters3D, PhysicsServer3D, Resource, RigidBody3D,
};
use godot::obj::Gd;
use godot::prelude::*;
use spectator_protocol::query::{
    ChildData, DetailLevel, EntityData, FindBy, FrameInfoResponse, GetNodeInspectParams,
    GetSceneTreeParams, GetSnapshotDataParams, InspectCategory, InspectChild, InspectPhysics,
    InspectScript, InspectSignals, InspectState, InspectTransform, NearbyEntityRaw,
    NavPathResponse, NodeInspectResponse, PerspectiveData, PerspectiveParam, PhysicsEntityData,
    RaycastResponse, ResolveNodeResponse, SceneTreeAction, SnapshotResponse, SpatialContextRaw,
    TransformEntityData, TreeInclude,
};

/// State for deferred frame advance (set by action_handler, read by tcp_server).
#[derive(Default)]
pub struct AdvanceState {
    /// Number of physics frames remaining to advance.
    pub remaining: u32,
    /// Request ID waiting for advance completion.
    pub pending_id: Option<String>,
}


#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorCollector {
    base: Base<Node>,
    pub advance_state: std::cell::RefCell<AdvanceState>,
}

#[godot_api]
impl INode for SpectatorCollector {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            advance_state: std::cell::RefCell::new(AdvanceState::default()),
        }
    }
}

#[godot_api]
impl SpectatorCollector {
    /// GDScript-callable wrapper (used for testing).
    #[func]
    pub fn collect_snapshot_dict(&self, _params_json: GString) -> VarDictionary {
        VarDictionary::new()
    }
}

impl SpectatorCollector {
    /// Collect scene snapshot data based on the provided parameters.
    pub fn collect_snapshot(&self, params: &GetSnapshotDataParams) -> SnapshotResponse {
        let tree = match self.base().get_tree() {
            Some(t) => t,
            None => return snapshot_empty(),
        };
        let root = match tree.get_current_scene() {
            Some(r) => r,
            None => return snapshot_empty(),
        };

        let perspective = self.resolve_perspective(&params.perspective);
        let frame_info = self.get_frame_info();

        let mut entities = Vec::new();
        self.collect_entities_recursive(&root, params, &mut entities);

        SnapshotResponse {
            frame: frame_info.frame,
            timestamp_ms: frame_info.timestamp_ms,
            perspective,
            entities,
        }
    }

    /// Resolve perspective from camera, node, or point.
    fn resolve_perspective(&self, param: &PerspectiveParam) -> PerspectiveData {
        match param {
            PerspectiveParam::Camera => {
                if let Some(vp) = self.base().get_viewport() {
                    if let Some(camera) = vp.get_camera_3d() {
                        let pos = camera.get_global_position();
                        let rot = camera.get_global_rotation_degrees();
                        // Forward in Godot is -Z; col_c() is the local +Z column
                        let fwd = camera.get_global_transform().basis.col_c();
                        return PerspectiveData {
                            position: vec3(pos),
                            rotation_deg: vec3(rot),
                            forward: vec3(-fwd),
                        };
                    }
                }
                PerspectiveData {
                    position: vec![0.0, 0.0, 0.0],
                    rotation_deg: vec![0.0, 0.0, 0.0],
                    forward: vec![0.0, 0.0, -1.0],
                }
            }
            PerspectiveParam::Node { path } => {
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
                PerspectiveData {
                    position: vec![0.0, 0.0, 0.0],
                    rotation_deg: vec![0.0, 0.0, 0.0],
                    forward: vec![0.0, 0.0, -1.0],
                }
            }
            PerspectiveParam::Point { position } => PerspectiveData {
                position: position.clone(),
                rotation_deg: vec![0.0, 0.0, 0.0],
                forward: vec![0.0, 0.0, -1.0],
            },
        }
    }

    /// Recursively collect entity data from the scene tree.
    fn collect_entities_recursive(
        &self,
        node: &Gd<Node>,
        params: &GetSnapshotDataParams,
        entities: &mut Vec<EntityData>,
    ) {
        if self.is_spectator_node(node) {
            return;
        }

        if let Ok(node3d) = node.clone().try_cast::<Node3D>() {
            if self.should_collect(&node3d, params) {
                let entity = self.collect_single_entity(&node3d, params);
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

    /// Check if a node should be collected based on filters.
    fn should_collect(&self, node: &Gd<Node3D>, params: &GetSnapshotDataParams) -> bool {
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

    /// Collect data for a single entity.
    fn collect_single_entity(&self, node: &Gd<Node3D>, params: &GetSnapshotDataParams) -> EntityData {
        let pos = node.get_global_position();
        let rot = node.get_global_rotation_degrees();
        let class_name = node.get_class().to_string();
        let node_ref: Gd<Node> = node.clone().upcast();

        let velocity = self.get_velocity(node);
        let groups = self.get_groups(&node_ref);
        let visible = node.is_visible_in_tree();
        let state = self.get_exported_state(&node_ref);

        let mut entity = EntityData {
            path: self.get_relative_path(&node_ref),
            class: class_name,
            position: vec3(pos),
            rotation_deg: vec3(rot),
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

        // Full fields
        if params.detail == DetailLevel::Full {
            entity.children = self.get_children(&node_ref);
            entity.script = self.get_script_path(&node_ref);
            entity.signals_connected = self.get_connected_signals(&node_ref);
            entity.physics = self.get_physics_data(node);
            entity.transform = Some(self.get_transform_data(node));
            entity.all_exported_vars = Some(self.get_exported_state(&node_ref));
        }

        entity
    }

    /// Get the velocity of a node, if it's a physics body.
    fn get_velocity(&self, node: &Gd<Node3D>) -> Vec<f64> {
        if let Ok(body) = node.clone().try_cast::<CharacterBody3D>() {
            return vec3(body.get_velocity());
        }
        if let Ok(body) = node.clone().try_cast::<RigidBody3D>() {
            return vec3(body.get_linear_velocity());
        }
        vec![0.0, 0.0, 0.0]
    }

    /// Get all groups a node belongs to (excluding internal Godot groups).
    fn get_groups(&self, node: &Gd<Node>) -> Vec<String> {
        let groups: Array<StringName> = node.get_groups();
        let mut result = Vec::new();
        for i in 0..groups.len() {
            let group = groups.get(i).unwrap_or_default().to_string();
            if !group.starts_with('_') {
                result.push(group);
            }
        }
        result
    }

    /// Get exported variable state (@export vars).
    fn get_exported_state(&self, node: &Gd<Node>) -> serde_json::Map<String, serde_json::Value> {
        let mut state = serde_json::Map::new();
        let properties: Array<VarDictionary> = node.get_property_list();

        for i in 0..properties.len() {
            let Some(prop) = properties.get(i) else { continue };
            let usage = prop
                .get(GString::from("usage"))
                .and_then(|v| v.try_to::<i64>().ok())
                .unwrap_or(0);
            let name = prop
                .get(GString::from("name"))
                .and_then(|v| v.try_to::<GString>().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();

            if name.is_empty() {
                continue;
            }

            // PROPERTY_USAGE_SCRIPT_VARIABLE (4096) AND PROPERTY_USAGE_EDITOR (4) — exported vars
            if usage & (4096 | 4) == (4096 | 4) {
                let value = node.get(&name);
                if let Some(json_value) = variant_to_json(&value) {
                    state.insert(name, json_value);
                }
            }
        }

        state
    }

    /// Get immediate children info.
    fn get_children(&self, node: &Gd<Node>) -> Vec<ChildData> {
        let count = node.get_child_count();
        let mut children = Vec::new();
        for i in 0..count {
            if let Some(child) = node.get_child(i) {
                children.push(ChildData {
                    name: child.get_name().to_string(),
                    class: child.get_class().to_string(),
                });
            }
        }
        children
    }

    /// Get script path if a script is attached.
    fn get_script_path(&self, node: &Gd<Node>) -> Option<String> {
        let script: Gd<Resource> = node.get_script()?.upcast();
        let path = script.get_path().to_string();
        if path.is_empty() { None } else { Some(path) }
    }

    /// Get names of signals that have connections.
    fn get_connected_signals(&self, node: &Gd<Node>) -> Vec<String> {
        let signals: Array<VarDictionary> = node.get_signal_list();
        let mut result = Vec::new();
        for i in 0..signals.len() {
            let Some(sig) = signals.get(i) else { continue };
            let name = sig
                .get(GString::from("name"))
                .and_then(|v| v.try_to::<GString>().ok())
                .map(|s| s.to_string())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let connections = node.get_signal_connection_list(name.as_str());
            if connections.len() > 0 {
                result.push(name);
            }
        }
        result
    }

    /// Get physics data for CharacterBody3D.
    fn get_physics_data(&self, node: &Gd<Node3D>) -> Option<PhysicsEntityData> {
        if let Ok(body) = node.clone().try_cast::<CharacterBody3D>() {
            let v = body.get_velocity();
            let on_floor = body.is_on_floor();
            let floor_normal = if on_floor {
                Some(vec3(body.get_floor_normal()))
            } else {
                None
            };
            let phys: Gd<PhysicsBody3D> = body.upcast();
            let layer = phys.get_collision_layer();
            let mask = phys.get_collision_mask();
            return Some(PhysicsEntityData {
                velocity: vec3(v),
                on_floor,
                floor_normal,
                collision_layer: layer,
                collision_mask: mask,
            });
        }
        None
    }

    /// Get full transform data.
    fn get_transform_data(&self, node: &Gd<Node3D>) -> TransformEntityData {
        let t = node.get_global_transform();
        let origin = t.origin;
        let basis = t.basis;
        let scale = node.get_scale();
        TransformEntityData {
            origin: vec3(origin),
            basis: vec![
                vec3(basis.col_a()),
                vec3(basis.col_b()),
                vec3(basis.col_c()),
            ],
            scale: vec3(scale),
        }
    }

    /// Get the relative path of a node from the current scene root.
    fn get_relative_path(&self, node: &Gd<Node>) -> String {
        if let Some(tree) = self.base().get_tree() {
            if let Some(root) = tree.get_current_scene() {
                return root.get_path_to(node).to_string();
            }
        }
        node.get_name().to_string()
    }

    /// Check if a node is part of Spectator's own infrastructure.
    fn is_spectator_node(&self, node: &Gd<Node>) -> bool {
        let name = node.get_name().to_string();
        name == "SpectatorRuntime"
            || name.starts_with("SpectatorTCPServer")
            || name.starts_with("SpectatorCollector")
            || name.starts_with("SpectatorRecorder")
            || node.is_in_group("spectator_internal")
    }

    // -------------------------------------------------------------------------
    // M2: Node Inspection
    // -------------------------------------------------------------------------

    /// Collect deep inspection data for a single node.
    pub fn inspect_node(&self, params: &GetNodeInspectParams) -> Result<NodeInspectResponse, String> {
        let tree = self.base().get_tree().ok_or("No scene tree available")?;
        let root = tree.get_current_scene().ok_or("No current scene")?;
        let node: Gd<Node> = root
            .try_get_node_as(params.path.as_str())
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
        if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
            let global = n3d.get_global_position();
            let global_rot = n3d.get_global_rotation_degrees();
            let local = n3d.get_position();
            let scale = n3d.get_scale();
            InspectTransform {
                global_origin: vec3(global),
                global_rotation_deg: vec3(global_rot),
                local_origin: vec3(local),
                scale: vec3(scale),
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
        if let Ok(body) = node.clone().try_cast::<CharacterBody3D>() {
            let v = body.get_velocity();
            let speed = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt() as f64;
            let on_floor = body.is_on_floor();
            let on_wall = body.is_on_wall();
            let on_ceiling = body.is_on_ceiling();
            let floor_normal = if on_floor {
                Some(vec3(body.get_floor_normal()))
            } else {
                None
            };
            let phys: Gd<PhysicsBody3D> = body.upcast();
            return Some(InspectPhysics {
                velocity: vec3(v),
                speed,
                on_floor,
                on_wall,
                on_ceiling,
                floor_normal,
                collision_layer: phys.get_collision_layer(),
                collision_mask: phys.get_collision_mask(),
            });
        }
        if let Ok(body) = node.clone().try_cast::<RigidBody3D>() {
            let v = body.get_linear_velocity();
            let speed = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt() as f64;
            let phys: Gd<PhysicsBody3D> = body.upcast();
            return Some(InspectPhysics {
                velocity: vec3(v),
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
            exported: self.get_exported_state(node),
            internal: None,
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
                if let Ok(cs) = child.clone().try_cast::<godot::classes::CollisionShape3D>() {
                    if let Some(shape) = cs.get_shape() {
                        props.insert(
                            "shape".to_string(),
                            serde_json::Value::String(shape.get_class().to_string()),
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
                // NavigationAgent3D is not exposed as a gdext class in api-4-2;
                // props are skipped for now.
            }
            "Area3D" => {
                if let Ok(area) = child.clone().try_cast::<godot::classes::Area3D>() {
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
            if name.is_empty() {
                continue;
            }
            let conns = node.get_signal_connection_list(name.as_str());
            if conns.len() > 0 {
                let targets: Vec<serde_json::Value> = (0..conns.len())
                    .filter_map(|j| {
                        let conn = conns.get(j)?;
                        let callable = conn
                            .get(GString::from("callable"))
                            .and_then(|v| v.try_to::<godot::builtin::Callable>().ok())?;
                        let obj_name = callable
                            .object()
                            .map(|o| {
                                if let Ok(n) = o.clone().try_cast::<Node>() {
                                    self.get_relative_path(&n)
                                } else {
                                    format!("<{}>", o.get_class())
                                }
                            })
                            .unwrap_or_else(|| "<unknown>".to_string());
                        let method = callable.method_name().map(|n| n.to_string()).unwrap_or_default();
                        Some(serde_json::Value::String(format!("{obj_name}:{method}")))
                    })
                    .collect();
                connected.insert(name, serde_json::Value::Array(targets));
            }
        }

        InspectSignals {
            connected,
            recent_emissions: Vec::new(),
        }
    }

    fn collect_inspect_script(&self, node: &Gd<Node>) -> Option<InspectScript> {
        let script_variant = node.get_script()?;
        let script: Gd<Resource> = script_variant.upcast();
        let path = script.get_path().to_string();
        if path.is_empty() {
            return None;
        }

        let base_class = node.get_class().to_string();

        let methods = if let Ok(mut gd_script) = script.clone().try_cast::<godot::classes::Script>() {
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

        let mut extends_chain = Vec::new();
        let mut current_class = base_class.clone();
        extends_chain.push(current_class.clone());
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
    fn collect_spatial_context_raw(&self, node: &Gd<Node>) -> SpatialContextRaw {
        let node3d = match node.clone().try_cast::<Node3D>() {
            Ok(n) => n,
            Err(_) => {
                return SpatialContextRaw {
                    nearby: Vec::new(),
                    in_areas: Vec::new(),
                    camera_visible: false,
                    camera_distance: 0.0,
                    node_position: Vec::new(),
                    node_forward: Vec::new(),
                }
            }
        };

        let pos = node3d.get_global_position();
        let fwd_col = node3d.get_global_transform().basis.col_c();
        let node_position = vec3(pos);
        let node_forward = vec3(-fwd_col);

        let (camera_visible, camera_distance) = if let Some(vp) = self.base().get_viewport() {
            if let Some(camera) = vp.get_camera_3d() {
                let cam_pos = camera.get_global_position();
                let dist = pos.distance_to(cam_pos) as f64;
                let visible = node3d.is_visible_in_tree();
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
                self.collect_nearby_recursive(&root, &pos, node, &mut nearby, 20.0);
                nearby.sort_by(|a, b| {
                    let da = position_distance(&a.position, &node_position);
                    let db = position_distance(&b.position, &node_position);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
                nearby.truncate(10);
            }
        }

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
        if self.is_spectator_node(node) {
            return;
        }
        if node.instance_id() == exclude.instance_id() {
            return;
        }

        if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
            let pos = n3d.get_global_position();
            let dist = pos.distance_to(*target_pos) as f64;
            if dist <= radius {
                let node_ref: Gd<Node> = n3d.clone().upcast();
                result.push(NearbyEntityRaw {
                    path: self.get_relative_path(&node_ref),
                    class: node_ref.get_class().to_string(),
                    position: vec3(pos),
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

    // -------------------------------------------------------------------------
    // M2: Scene Tree Queries
    // -------------------------------------------------------------------------

    /// Handle scene tree queries.
    pub fn query_scene_tree(
        &self,
        params: &GetSceneTreeParams,
    ) -> Result<serde_json::Value, String> {
        match params.action {
            SceneTreeAction::Roots => self.scene_tree_roots(&params.include),
            SceneTreeAction::Children => {
                let path = params
                    .node
                    .as_ref()
                    .ok_or("'node' is required for 'children' action")?;
                self.scene_tree_children(path, &params.include)
            }
            SceneTreeAction::Subtree => {
                let path = params
                    .node
                    .as_ref()
                    .ok_or("'node' is required for 'subtree' action")?;
                self.scene_tree_subtree(path, params.depth, &params.include)
            }
            SceneTreeAction::Ancestors => {
                let path = params
                    .node
                    .as_ref()
                    .ok_or("'node' is required for 'ancestors' action")?;
                self.scene_tree_ancestors(path, &params.include)
            }
            SceneTreeAction::Find => {
                let find_by = params
                    .find_by
                    .as_ref()
                    .ok_or("'find_by' is required for 'find' action")?;
                let find_value = params
                    .find_value
                    .as_ref()
                    .ok_or("'find_value' is required for 'find' action")?;
                self.scene_tree_find(*find_by, find_value, &params.include)
            }
        }
    }

    fn scene_tree_roots(
        &self,
        include: &[TreeInclude],
    ) -> Result<serde_json::Value, String> {
        let tree = self.base().get_tree().ok_or("No scene tree")?;
        let root = tree.get_root().ok_or("No root node")?;

        let mut roots = Vec::new();
        let count = root.get_child_count();
        for i in 0..count {
            if let Some(child) = root.get_child(i) {
                if self.is_spectator_node(&child) {
                    continue;
                }
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
                if self.is_spectator_node(&child) {
                    continue;
                }
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
        let total = self.count_nodes(&node);
        let tree = self.build_subtree(&node, max_depth, 0, include);
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
                    if self.is_spectator_node(&child) {
                        continue;
                    }
                    let name = child.get_name().to_string();
                    let child_tree =
                        self.build_subtree(&child, max_depth, current_depth + 1, include);
                    children.insert(name, child_tree);
                }
            }
            if !children.is_empty() {
                if let serde_json::Value::Object(ref mut map) = info {
                    map.insert(
                        "children".to_string(),
                        serde_json::Value::Object(children),
                    );
                }
            }
        } else if node.get_child_count() > 0 {
            if let serde_json::Value::Object(ref mut map) = info {
                map.insert(
                    "children".to_string(),
                    serde_json::json!({"...": "depth_limit_reached"}),
                );
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

        ancestors.push(self.node_info(&node, include));

        let mut current: Gd<Node> = node;
        while let Some(parent) = current.get_parent() {
            if parent.get_parent().is_none() {
                break;
            }
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
        let tree = self.base().get_tree().ok_or("No scene tree")?;
        let root = tree.get_current_scene().ok_or("No current scene")?;

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
        if self.is_spectator_node(node) {
            return;
        }

        let matches = match find_by {
            FindBy::Name => node.get_name().to_string().contains(value),
            FindBy::Class => node.get_class().to_string() == value,
            FindBy::Group => node.is_in_group(value),
            FindBy::Script => self
                .get_script_path(node)
                .as_deref()
                .map(|p| p == value)
                .unwrap_or(false),
        };

        if matches {
            let mut info = self.node_info(node, include);
            if let serde_json::Value::Object(ref mut map) = info {
                map.insert(
                    "path".to_string(),
                    serde_json::Value::String(self.get_relative_path(node)),
                );
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
    fn node_info(&self, node: &Gd<Node>, include: &[TreeInclude]) -> serde_json::Value {
        let mut info = serde_json::Map::new();
        let name = node.get_name().to_string();
        info.insert("name".to_string(), serde_json::json!(name));

        for inc in include {
            match inc {
                TreeInclude::Class => {
                    info.insert(
                        "class".to_string(),
                        serde_json::json!(node.get_class().to_string()),
                    );
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
                        info.insert(
                            "visible".to_string(),
                            serde_json::json!(n3d.is_visible_in_tree()),
                        );
                    }
                }
                TreeInclude::ProcessMode => {
                    let mode = node.get_process_mode();
                    info.insert(
                        "process_mode".to_string(),
                        serde_json::json!(format!("{:?}", mode)),
                    );
                }
            }
        }

        serde_json::Value::Object(info)
    }

    /// Resolve a node path to a Gd<Node>.
    fn resolve_node(&self, path: &str) -> Result<Gd<Node>, String> {
        let tree = self.base().get_tree().ok_or("No scene tree")?;
        let root = tree.get_current_scene().ok_or("No current scene")?;
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

    /// Get current frame info.
    pub fn get_frame_info(&self) -> FrameInfoResponse {
        let engine = Engine::singleton();
        let frame = engine.get_physics_frames() as u64;
        let ticks = engine.get_physics_ticks_per_second() as u64;
        let timestamp_ms = if ticks > 0 { (frame * 1000) / ticks } else { 0 };
        let delta = if ticks > 0 { 1.0 / ticks as f64 } else { 0.0 };
        FrameInfoResponse {
            frame,
            timestamp_ms,
            delta,
        }
    }

    /// Public wrapper for resolve_node (used by action_handler).
    pub fn resolve_node_public(&self, path: &str) -> Result<Gd<Node>, String> {
        self.resolve_node(path)
    }

    /// Perform a physics raycast from one point to another.
    pub fn raycast(
        &self,
        from: Vector3,
        to: Vector3,
        collision_mask: Option<u32>,
    ) -> Result<RaycastResponse, String> {
        let tree = self.base().get_tree().ok_or("Not in scene tree")?;
        let world = tree
            .get_root()
            .ok_or("No root")?
            .get_world_3d()
            .ok_or("No World3D — is this a 3D scene?")?;
        let space = world.get_space();
        let mut physics_server = PhysicsServer3D::singleton();
        let mut direct_state = physics_server
            .space_get_direct_state(space)
            .ok_or("Could not get physics direct state")?;

        let mut query =
            PhysicsRayQueryParameters3D::create(from, to).ok_or("Could not create ray query")?;
        if let Some(mask) = collision_mask {
            query.set_collision_mask(mask);
        }

        let result = direct_state.intersect_ray(&query);
        let total_distance = from.distance_to(to) as f64;

        if result.is_empty() {
            Ok(RaycastResponse {
                clear: true,
                blocked_by: None,
                blocked_at: None,
                total_distance,
                clear_distance: total_distance,
            })
        } else {
            let hit_pos: Vector3 = result
                .get("position")
                .map(|v| v.to::<Vector3>())
                .unwrap_or(Vector3::ZERO);
            let blocked_by = result.get("collider").and_then(|v| {
                v.try_to::<Gd<godot::classes::Object>>()
                    .ok()
                    .and_then(|obj| obj.try_cast::<Node>().ok())
                    .map(|n| self.get_relative_path(&n))
            });

            Ok(RaycastResponse {
                clear: false,
                blocked_by,
                blocked_at: Some(vec![hit_pos.x as f64, hit_pos.y as f64, hit_pos.z as f64]),
                total_distance,
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
        let maps = nav_server.get_maps();
        if maps.len() == 0 {
            return Err(
                "No navigation maps available. Is NavigationServer3D active?".into(),
            );
        }
        let map = maps.get(0).ok_or("No navigation map at index 0")?;
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
            path_ratio: if straight_distance > 0.0 {
                nav_distance / straight_distance
            } else {
                1.0
            },
            path_points: path.len() as u32,
            traversable,
        })
    }

    /// Resolve a node path to its position, forward vector, and groups.
    pub fn resolve_node_position(&self, path: &str) -> Result<ResolveNodeResponse, String> {
        let node = self.resolve_node(path)?;
        if let Ok(n3d) = node.clone().try_cast::<Node3D>() {
            let pos = n3d.get_global_position();
            // Forward in Godot is -Z; col_c() is the local +Z column
            let fwd = -n3d.get_global_basis().col_c();
            Ok(ResolveNodeResponse {
                position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
                forward: vec![fwd.x as f64, fwd.y as f64, fwd.z as f64],
                groups: self.get_groups(&node),
            })
        } else if let Ok(n2d) = node.clone().try_cast::<Node2D>() {
            let pos = n2d.get_global_position();
            Ok(ResolveNodeResponse {
                position: vec![pos.x as f64, pos.y as f64],
                forward: vec![1.0, 0.0],
                groups: self.get_groups(&node),
            })
        } else {
            Err(format!("Node '{path}' is not a Node3D or Node2D"))
        }
    }

    /// Set the advance state for frame-stepping.
    pub fn set_advance_state(&self, remaining: u32, pending_id: Option<String>) {
        let mut state = self.advance_state.borrow_mut();
        state.remaining = remaining;
        state.pending_id = pending_id;
    }
}

/// Convert a Godot `Vector3` to a `Vec<f64>`.
fn vec3(v: Vector3) -> Vec<f64> {
    vec![v.x as f64, v.y as f64, v.z as f64]
}

fn snapshot_empty() -> SnapshotResponse {
    SnapshotResponse {
        frame: 0,
        timestamp_ms: 0,
        perspective: PerspectiveData {
            position: vec![0.0, 0.0, 0.0],
            rotation_deg: vec![0.0, 0.0, 0.0],
            forward: vec![0.0, 0.0, -1.0],
        },
        entities: Vec::new(),
    }
}

/// Euclidean distance between two position arrays.
fn position_distance(a: &[f64], b: &[f64]) -> f64 {
    let dx = a.first().unwrap_or(&0.0) - b.first().unwrap_or(&0.0);
    let dy = a.get(1).unwrap_or(&0.0) - b.get(1).unwrap_or(&0.0);
    let dz = a.get(2).unwrap_or(&0.0) - b.get(2).unwrap_or(&0.0);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Convert a Godot Variant to a JSON value.
/// Returns None for types we can't meaningfully represent.
pub(crate) fn variant_to_json(v: &Variant) -> Option<serde_json::Value> {
    match v.get_type() {
        VariantType::NIL => Some(serde_json::Value::Null),
        VariantType::BOOL => Some(serde_json::Value::Bool(v.to::<bool>())),
        VariantType::INT => Some(serde_json::json!(v.to::<i64>())),
        VariantType::FLOAT => {
            let f = v.to::<f64>();
            serde_json::Number::from_f64(f).map(serde_json::Value::Number)
        }
        VariantType::STRING | VariantType::STRING_NAME | VariantType::NODE_PATH => {
            Some(serde_json::Value::String(v.to::<GString>().to_string()))
        }
        VariantType::VECTOR2 => {
            let vec = v.to::<Vector2>();
            Some(serde_json::json!([vec.x, vec.y]))
        }
        VariantType::VECTOR3 => {
            let vec = v.to::<Vector3>();
            Some(serde_json::json!([vec.x, vec.y, vec.z]))
        }
        VariantType::COLOR => {
            let c = v.to::<godot::builtin::Color>();
            Some(serde_json::json!([c.r, c.g, c.b, c.a]))
        }
        VariantType::ARRAY => {
            let arr = v.to::<Array<Variant>>();
            let items: Vec<serde_json::Value> = (0..arr.len())
                .filter_map(|i| arr.get(i).and_then(|v| variant_to_json(&v)))
                .collect();
            Some(serde_json::Value::Array(items))
        }
        VariantType::DICTIONARY => {
            let dict = v.to::<VarDictionary>();
            let mut map = serde_json::Map::new();
            for key in dict.keys_array().iter_shared() {
                let key_str = key.to::<GString>().to_string();
                if let Some(val) = dict.get(key).and_then(|v| variant_to_json(&v)) {
                    map.insert(key_str, val);
                }
            }
            Some(serde_json::Value::Object(map))
        }
        _ => Some(serde_json::Value::String(format!("{v}"))),
    }
}
