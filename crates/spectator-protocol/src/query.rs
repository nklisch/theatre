#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for the `get_snapshot_data` query method.
/// Sent by the server to the addon to collect scene data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSnapshotDataParams {
    /// Camera/node/point perspective.
    pub perspective: PerspectiveParam,
    /// Max radius from focal point.
    pub radius: f64,
    /// Whether to include offscreen nodes.
    pub include_offscreen: bool,
    /// Group filter (empty = all groups).
    #[serde(default)]
    pub groups: Vec<String>,
    /// Class filter (empty = all classes).
    #[serde(default)]
    pub class_filter: Vec<String>,
    /// What detail to collect.
    pub detail: DetailLevel,
    /// Whether to include internal (non-exported) variables.
    #[serde(default)]
    pub expose_internals: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PerspectiveParam {
    Camera,
    Node { path: String },
    Point { position: Vec<f64> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    Summary,
    #[default]
    Standard,
    Full,
}

/// Response data from `get_snapshot_data`.
/// This is the raw data the addon sends back — the server does all processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotResponse {
    /// Current frame number.
    pub frame: u64,
    /// Timestamp in ms since game start.
    pub timestamp_ms: u64,
    /// Perspective position and rotation.
    pub perspective: PerspectiveData,
    /// All collected entities (sorted by distance is NOT the addon's job).
    pub entities: Vec<EntityData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveData {
    pub position: Vec<f64>,
    pub rotation_deg: Vec<f64>,
    pub forward: Vec<f64>,
}

/// Raw entity data sent by the addon.
/// Simpler than core::RawEntityData — just engine data, no spatial reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    pub path: String,
    pub class: String,
    pub position: Vec<f64>,
    pub rotation_deg: Vec<f64>,
    pub velocity: Vec<f64>,
    pub groups: Vec<String>,
    pub visible: bool,
    /// Exported variable state.
    pub state: serde_json::Map<String, serde_json::Value>,
    // -- standard+ fields --
    #[serde(default)]
    pub signals_recent: Vec<RecentSignalData>,
    // -- full fields --
    #[serde(default)]
    pub children: Vec<ChildData>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub signals_connected: Vec<String>,
    #[serde(default)]
    pub physics: Option<PhysicsEntityData>,
    #[serde(default)]
    pub transform: Option<TransformEntityData>,
    #[serde(default)]
    pub all_exported_vars: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentSignalData {
    pub signal: String,
    pub frame: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildData {
    pub name: String,
    pub class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsEntityData {
    pub velocity: Vec<f64>,
    pub on_floor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor_normal: Option<Vec<f64>>,
    pub collision_layer: u32,
    pub collision_mask: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformEntityData {
    pub origin: Vec<f64>,
    pub basis: Vec<Vec<f64>>,
    pub scale: Vec<f64>,
}

/// Parameters for `get_frame_info` query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFrameInfoParams {}

/// Response for `get_frame_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameInfoResponse {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub delta: f64,
}

// --- spatial_inspect protocol types ---

/// Parameters for `get_node_inspect` query method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeInspectParams {
    /// Node path relative to scene root.
    pub path: String,
    /// Which data categories to collect.
    pub include: Vec<InspectCategory>,
    /// Whether to include internal (non-exported) variables.
    #[serde(default)]
    pub expose_internals: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum InspectCategory {
    Transform,
    Physics,
    State,
    Children,
    Signals,
    Script,
    SpatialContext,
    Resources,
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
    /// Resource data from node and immediate children (opt-in via include).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<InspectResources>,
}

/// Resource data collected from a node and its immediate children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectResources {
    /// Mesh data from MeshInstance3D/MeshInstance2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<MeshResourceData>,
    /// Collision shape data from CollisionShape3D/CollisionShape2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collision_shapes: Vec<CollisionShapeData>,
    /// Animation player data from AnimationPlayer children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animation_players: Vec<AnimationPlayerData>,
    /// Navigation agent data from NavigationAgent3D/2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub navigation_agents: Vec<NavigationAgentData>,
    /// Sprite data from Sprite2D/Sprite3D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sprites: Vec<SpriteData>,
    /// Particle system data from GPUParticles3D/2D children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub particles: Vec<ParticleData>,
    /// Shader parameters from ShaderMaterial on the node or mesh children.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub shader_params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshResourceData {
    /// Name of the child node holding the mesh.
    pub child: String,
    /// Resource path (e.g. "res://models/scout.tres") or null if inline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Mesh class name (e.g. "ArrayMesh", "BoxMesh", "SphereMesh").
    #[serde(rename = "type")]
    pub mesh_type: String,
    /// Number of surfaces in the mesh.
    pub surface_count: u32,
    /// Material overrides per surface index.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_overrides: Vec<MaterialOverrideData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialOverrideData {
    /// Surface index.
    pub surface: u32,
    /// Material resource path or null if inline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// Material class name (e.g. "StandardMaterial3D", "ShaderMaterial").
    #[serde(rename = "type")]
    pub material_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollisionShapeData {
    /// Name of the child node holding the shape.
    pub child: String,
    /// Shape class name (e.g. "CapsuleShape3D", "BoxShape3D", "CircleShape2D").
    #[serde(rename = "type")]
    pub shape_type: String,
    /// Shape dimensions as key-value pairs (e.g. {"radius": 0.5, "height": 1.8}).
    pub dimensions: serde_json::Map<String, serde_json::Value>,
    /// Whether the shape is an inline resource (true) or loaded from a file (false).
    pub inline: bool,
    /// Disabled flag from the CollisionShape node.
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationPlayerData {
    /// Name of the AnimationPlayer child node.
    pub child: String,
    /// Currently playing animation name (null if stopped).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_animation: Option<String>,
    /// List of available animation names.
    pub animations: Vec<String>,
    /// Current playback position in seconds.
    pub position_sec: f64,
    /// Length of current animation in seconds (0.0 if stopped).
    pub length_sec: f64,
    /// Whether the current animation loops.
    pub looping: bool,
    /// Whether the player is currently playing.
    pub playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationAgentData {
    /// Name of the NavigationAgent child node.
    pub child: String,
    /// Target position the agent is navigating toward.
    pub target_position: Vec<f64>,
    /// Whether the target has been reached.
    pub target_reached: bool,
    /// Remaining distance to the target.
    pub distance_remaining: f64,
    /// Path postprocessing mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_postprocessing: Option<String>,
    /// Whether avoidance is enabled.
    pub avoidance_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteData {
    /// Name of the Sprite child node.
    pub child: String,
    /// Texture resource path (null if no texture).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// Whether the sprite is visible.
    pub visible: bool,
    /// Flip flags.
    pub flip_h: bool,
    pub flip_v: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleData {
    /// Name of the particle system child node.
    pub child: String,
    /// Whether particles are currently emitting.
    pub emitting: bool,
    /// Number of particles.
    pub amount: i32,
    /// Process material resource path (null if inline/none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_material: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectTransform {
    pub global_position: Vec<f64>,
    pub global_rotation_deg: Vec<f64>,
    pub position: Vec<f64>,
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
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SceneTreeAction {
    Roots,
    Children,
    Subtree,
    Ancestors,
    Find,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FindBy {
    Name,
    Class,
    Group,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
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
    PathDistance { from: QueryOrigin, to: QueryOrigin },
    /// Get position and forward vector for a node (for server-side queries).
    ResolveNode { path: String },
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

// --- Signal subscription protocol types ---

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_params_round_trip() {
        let params = GetSnapshotDataParams {
            perspective: PerspectiveParam::Camera,
            radius: 50.0,
            include_offscreen: false,
            groups: vec!["enemies".to_string()],
            class_filter: vec![],
            detail: DetailLevel::Standard,
            expose_internals: false,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: GetSnapshotDataParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.radius, 50.0);
        assert_eq!(parsed.groups, vec!["enemies"]);
    }

    #[test]
    fn perspective_param_tagged_enum() {
        let p = PerspectiveParam::Node {
            path: "Player".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""type":"node""#));
        assert!(json.contains("Player"));

        let p2 = PerspectiveParam::Point {
            position: vec![1.0, 2.0, 3.0],
        };
        let json2 = serde_json::to_string(&p2).unwrap();
        assert!(json2.contains(r#""type":"point""#));
    }

    #[test]
    fn entity_data_optional_fields() {
        let json = r#"{
            "path": "enemies/scout",
            "class": "CharacterBody3D",
            "position": [1.0, 0.0, 2.0],
            "rotation_deg": [0.0, 45.0, 0.0],
            "velocity": [0.0, 0.0, 0.0],
            "groups": ["enemies"],
            "visible": true,
            "state": {}
        }"#;
        let entity: EntityData = serde_json::from_str(json).unwrap();
        assert_eq!(entity.path, "enemies/scout");
        assert!(entity.physics.is_none());
        assert!(entity.transform.is_none());
        assert!(entity.children.is_empty());
    }

    #[test]
    fn inspect_params_round_trip() {
        let params = GetNodeInspectParams {
            path: "enemies/scout_02".to_string(),
            include: vec![InspectCategory::Transform, InspectCategory::Physics],
            expose_internals: false,
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
    fn action_request_tagged_enum_serde() {
        let req = ActionRequest::Teleport {
            path: "enemy".into(),
            position: vec![5.0, 0.0, -3.0],
            rotation_deg: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""action":"teleport""#), "got: {json}");
        let parsed: ActionRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ActionRequest::Teleport { .. }));
    }

    #[test]
    fn action_request_pause_serde() {
        let req = ActionRequest::Pause { paused: true };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""action":"pause""#));
        let parsed: ActionRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ActionRequest::Pause { paused: true }));
    }

    #[test]
    fn action_response_round_trip() {
        let resp = ActionResponse {
            action: "teleport".into(),
            result: "ok".into(),
            details: serde_json::Map::from_iter([(
                "previous_position".into(),
                serde_json::json!([1.0, 2.0, 3.0]),
            )]),
            frame: 100,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ActionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.action, "teleport");
        assert_eq!(parsed.frame, 100);
    }

    #[test]
    fn query_origin_untagged_serde() {
        let node: QueryOrigin = serde_json::from_str(r#""player""#).unwrap();
        assert!(matches!(node, QueryOrigin::Node(s) if s == "player"));

        let pos: QueryOrigin = serde_json::from_str(r#"[1.0, 2.0, 3.0]"#).unwrap();
        assert!(matches!(pos, QueryOrigin::Position(v) if v.len() == 3));
    }

    #[test]
    fn spatial_query_request_raycast_serde() {
        let req = SpatialQueryRequest::Raycast {
            from: QueryOrigin::Node("player".into()),
            to: QueryOrigin::Position(vec![0.0, 0.0, 0.0]),
            collision_mask: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""query_type":"raycast""#));
        let parsed: SpatialQueryRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, SpatialQueryRequest::Raycast { .. }));
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
            resources: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("transform"));
        assert!(!json.contains("physics"));
        assert!(!json.contains("resources"));
    }

    #[test]
    fn inspect_resources_round_trip() {
        use serde_json::json;
        let resources = InspectResources {
            meshes: vec![MeshResourceData {
                child: "Mesh".into(),
                resource: Some("res://models/scout.tres".into()),
                mesh_type: "ArrayMesh".into(),
                surface_count: 3,
                material_overrides: vec![MaterialOverrideData {
                    surface: 0,
                    resource: Some("res://materials/skin.tres".into()),
                    material_type: "StandardMaterial3D".into(),
                }],
            }],
            collision_shapes: vec![CollisionShapeData {
                child: "CollisionShape3D".into(),
                shape_type: "CapsuleShape3D".into(),
                dimensions: {
                    let mut m = serde_json::Map::new();
                    m.insert("radius".into(), json!(0.5));
                    m.insert("height".into(), json!(1.8));
                    m
                },
                inline: true,
                disabled: false,
            }],
            animation_players: vec![],
            navigation_agents: vec![],
            sprites: vec![],
            particles: vec![],
            shader_params: serde_json::Map::new(),
        };
        let json = serde_json::to_value(&resources).unwrap();
        let back: InspectResources = serde_json::from_value(json).unwrap();
        assert_eq!(back.meshes.len(), 1);
        assert_eq!(back.meshes[0].surface_count, 3);
        assert_eq!(back.collision_shapes[0].dimensions["radius"], 0.5);
    }

    #[test]
    fn inspect_resources_empty_collections_omitted() {
        let resources = InspectResources {
            meshes: vec![],
            collision_shapes: vec![],
            animation_players: vec![],
            navigation_agents: vec![],
            sprites: vec![],
            particles: vec![],
            shader_params: serde_json::Map::new(),
        };
        let json = serde_json::to_string(&resources).unwrap();
        assert!(!json.contains("meshes"));
        assert!(!json.contains("collision_shapes"));
    }

    #[test]
    fn inspect_category_resources_deserializes() {
        let json = serde_json::json!("resources");
        let cat: InspectCategory = serde_json::from_value(json).unwrap();
        assert_eq!(cat, InspectCategory::Resources);
    }
}
