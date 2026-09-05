use serde::{Deserialize, Serialize};

/// Camera pose captured alongside a spatial frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CameraFrameData {
    pub position: Vec<f64>,
    pub quaternion: Vec<f64>,
    pub projection: u8,
    pub fov_deg: f64,
    pub ortho_size: f64,
    pub keep_aspect: u8,
    #[serde(default)]
    pub camera_path: String,
}

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
    // Keep this trailing: old MessagePack structs are eight-element arrays.
    #[serde(default)]
    pub movement: Option<MovementFrameData>,
}

/// Sampled during the recorder's physics callback, not an input event log.
/// Contact facts describe the body's last move_and_slide call, which may precede
/// the sampled action strengths depending on game callback ordering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MovementFrameData {
    pub input_actions: std::collections::BTreeMap<String, f32>,
    pub on_floor: bool,
    pub on_wall: bool,
    pub on_ceiling: bool,
    /// Absent when on_floor is false; Godot only defines it on a floor.
    pub floor_normal: Option<[f64; 3]>,
    pub real_velocity: [f64; 3],
    pub slide_contact_normals: Vec<[f64; 3]>,
    pub slide_contacts_truncated: bool,
}

/// Upper bound per selected body per spatial sample.
pub const MAX_SLIDE_CONTACT_NORMALS: usize = 8;

pub const MOVEMENT_SAMPLING_LIMITS: &str = "Sampled at the spatial recorder's physics callback cadence, not raw input events. Input strengths are global InputMap intent, not proof a body consumed them. Contact facts and real_velocity describe the last move_and_slide call; callback ordering can place it before the sampled input. Short presses between samples can be missed. Missing movement means disabled, unselected, or unavailable; missing action names were removed from InputMap. This is diagnostic evidence, not deterministic replay.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_eight_field_messagepack_arrays_remain_readable() {
        let old = (
            "player",
            "CharacterBody3D",
            vec![0.0; 3],
            vec![0.0; 3],
            vec![0.0; 3],
            Vec::<String>::new(),
            true,
            serde_json::Map::<String, serde_json::Value>::new(),
        );
        let bytes = rmp_serde::to_vec(&vec![old]).unwrap();
        let decoded: Vec<FrameEntityData> = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded[0].path, "player");
        assert!(decoded[0].movement.is_none());
    }

    #[test]
    fn movement_roundtrips_without_losing_zero_strength_or_contacts() {
        let movement = MovementFrameData {
            input_actions: [("move_forward".into(), 0.0)].into(),
            on_floor: true,
            on_wall: true,
            on_ceiling: false,
            floor_normal: Some([0.0, 1.0, 0.0]),
            real_velocity: [0.0; 3],
            slide_contact_normals: vec![[1.0, 0.0, 0.0]],
            slide_contacts_truncated: true,
        };
        let mut entity: FrameEntityData = serde_json::from_value(serde_json::json!({
            "path":"player", "class":"CharacterBody3D", "position":[],
            "rotation_deg":[], "velocity":[], "groups":[], "visible":true, "state":{}
        }))
        .unwrap();
        assert!(entity.movement.is_none());
        entity.movement = Some(movement.clone());
        let bytes = rmp_serde::to_vec(&entity).unwrap();
        let decoded: FrameEntityData = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(decoded.movement, Some(movement));
    }

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
            movement: None,
        };
        let encoded = rmp_serde::to_vec(&entity).unwrap();
        let decoded: FrameEntityData = rmp_serde::from_slice(&encoded).unwrap();
        assert_eq!(decoded.path, "player");
        assert_eq!(decoded.position, vec![1.0, 0.0, 2.0]);
        assert_eq!(decoded.state["health"], serde_json::json!(100));
    }
}
