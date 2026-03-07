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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    Summary,
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
        let p = PerspectiveParam::Node { path: "Player".to_string() };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains(r#""type":"node""#));
        assert!(json.contains("Player"));

        let p2 = PerspectiveParam::Point { position: vec![1.0, 2.0, 3.0] };
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
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("transform"));
        assert!(!json.contains("physics"));
    }
}
