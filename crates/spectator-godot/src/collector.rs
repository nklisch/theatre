use godot::builtin::{Array, GString, StringName, Variant, VarDictionary, Vector2, Vector3};
use godot::builtin::VariantType;
use godot::classes::{CharacterBody3D, Engine, Node, Node3D, PhysicsBody3D, Resource, RigidBody3D};
use godot::obj::Gd;
use godot::prelude::*;
use spectator_protocol::query::{
    ChildData, DetailLevel, EntityData, FrameInfoResponse, GetSnapshotDataParams, PerspectiveData,
    PerspectiveParam, PhysicsEntityData, SnapshotResponse, TransformEntityData,
};

/// Static class heuristic: these classes are treated as static by default.
const STATIC_CLASSES: &[&str] = &[
    "StaticBody3D",
    "StaticBody2D",
    "CSGShape3D",
    "CSGBox3D",
    "CSGCylinder3D",
    "CSGMesh3D",
    "CSGPolygon3D",
    "CSGSphere3D",
    "CSGTorus3D",
    "CSGCombiner3D",
    "MeshInstance3D",
    "GridMap",
    "WorldEnvironment",
    "DirectionalLight3D",
    "OmniLight3D",
    "SpotLight3D",
];

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SpectatorCollector {
    base: Base<Node>,
}

#[godot_api]
impl INode for SpectatorCollector {
    fn init(base: Base<Node>) -> Self {
        Self { base }
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
                            position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
                            rotation_deg: vec![rot.x as f64, rot.y as f64, rot.z as f64],
                            forward: vec![-fwd.x as f64, -fwd.y as f64, -fwd.z as f64],
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
                        position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
                        rotation_deg: vec![rot.x as f64, rot.y as f64, rot.z as f64],
                        forward: vec![-fwd.x as f64, -fwd.y as f64, -fwd.z as f64],
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
            position: vec![pos.x as f64, pos.y as f64, pos.z as f64],
            rotation_deg: vec![rot.x as f64, rot.y as f64, rot.z as f64],
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
            let v = body.get_velocity();
            return vec![v.x as f64, v.y as f64, v.z as f64];
        }
        if let Ok(body) = node.clone().try_cast::<RigidBody3D>() {
            let v = body.get_linear_velocity();
            return vec![v.x as f64, v.y as f64, v.z as f64];
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
                let n = body.get_floor_normal();
                Some(vec![n.x as f64, n.y as f64, n.z as f64])
            } else {
                None
            };
            let phys: Gd<PhysicsBody3D> = body.upcast();
            let layer = phys.get_collision_layer();
            let mask = phys.get_collision_mask();
            return Some(PhysicsEntityData {
                velocity: vec![v.x as f64, v.y as f64, v.z as f64],
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
            origin: vec![origin.x as f64, origin.y as f64, origin.z as f64],
            basis: vec![
                vec![
                    basis.col_a().x as f64,
                    basis.col_a().y as f64,
                    basis.col_a().z as f64,
                ],
                vec![
                    basis.col_b().x as f64,
                    basis.col_b().y as f64,
                    basis.col_b().z as f64,
                ],
                vec![
                    basis.col_c().x as f64,
                    basis.col_c().y as f64,
                    basis.col_c().z as f64,
                ],
            ],
            scale: vec![scale.x as f64, scale.y as f64, scale.z as f64],
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

/// Convert a Godot Variant to a JSON value.
/// Returns None for types we can't meaningfully represent.
fn variant_to_json(v: &Variant) -> Option<serde_json::Value> {
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
