use serde::{Deserialize, Serialize};

/// Compact entity snapshot stored as MessagePack in recording frame BLOBs.
/// This is the wire format agreed upon by stage-godot (writer) and
/// stage-server (reader). Changes here require coordinated updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameEntityData {
    pub path: String,
    pub class: String,
    pub position: Vec<f64>,
    pub rotation_deg: Vec<f64>,
    pub velocity: Vec<f64>,
    pub groups: Vec<String>,
    pub visible: bool,
    pub state: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_entity_data_roundtrips_msgpack() {
        let entity = FrameEntityData {
            path: "player".into(),
            class: "CharacterBody3D".into(),
            position: vec![1.0, 0.0, 2.0],
            rotation_deg: vec![0.0, 45.0, 0.0],
            velocity: vec![0.0, 0.0, 0.0],
            groups: vec!["player".into()],
            visible: true,
            state: serde_json::from_str("{\"health\": 100}").unwrap(),
        };
        let encoded = rmp_serde::to_vec(&entity).unwrap();
        let decoded: FrameEntityData = rmp_serde::from_slice(&encoded).unwrap();
        assert_eq!(decoded.path, "player");
        assert_eq!(decoded.position, vec![1.0, 0.0, 2.0]);
        assert_eq!(decoded.state["health"], serde_json::json!(100));
    }
}
