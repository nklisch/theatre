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
}
