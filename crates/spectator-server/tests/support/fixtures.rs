/// Canonical test scene data with known positions.
use serde_json::json;
use spectator_protocol::query::{
    EntityData, PerspectiveData, SnapshotResponse,
};

/// A deterministic 3D scene: camera at (0,5,10) looking toward origin,
/// player at origin, enemy north, wall east, coin south.
pub fn mock_scene_3d() -> SnapshotResponse {
    SnapshotResponse {
        frame: 100,
        timestamp_ms: 1667,
        perspective: PerspectiveData {
            position: vec![0.0, 5.0, 10.0],
            rotation_deg: vec![-15.0, 0.0, 0.0],
            forward: vec![0.0, -0.259, -0.966],
        },
        entities: vec![
            entity("Player", "CharacterBody3D", [0.0, 0.0, 0.0], &["player"]),
            entity_with_state(
                "enemies/Scout",
                "CharacterBody3D",
                [0.0, 0.0, -5.0],
                &["enemies"],
                &[("health", json!(80))],
            ),
            entity("walls/EastWall", "StaticBody3D", [3.0, 0.0, 0.0], &["walls"]),
            entity("items/Coin", "Area3D", [0.0, 0.0, 2.0], &["collectibles"]),
            entity("Camera3D", "Camera3D", [0.0, 5.0, 10.0], &[]),
        ],
    }
}

/// A deterministic 2D scene.
pub fn mock_scene_2d() -> SnapshotResponse {
    SnapshotResponse {
        frame: 50,
        timestamp_ms: 833,
        perspective: PerspectiveData {
            position: vec![0.0, 0.0],
            rotation_deg: vec![0.0],
            forward: vec![1.0, 0.0],
        },
        entities: vec![
            entity_2d("Player", "CharacterBody2D", [100.0, 300.0], &["player"]),
            entity_2d("Enemy", "CharacterBody2D", [400.0, 300.0], &["enemies"]),
            entity_2d("Platform", "StaticBody2D", [250.0, 400.0], &["terrain"]),
        ],
    }
}

pub fn entity(path: &str, class: &str, pos: [f64; 3], groups: &[&str]) -> EntityData {
    EntityData {
        path: path.into(),
        class: class.into(),
        position: pos.to_vec(),
        rotation_deg: vec![0.0, 0.0, 0.0],
        velocity: vec![0.0, 0.0, 0.0],
        groups: groups.iter().map(|s| s.to_string()).collect(),
        visible: true,
        state: Default::default(),
        signals_recent: vec![],
        children: vec![],
        script: None,
        signals_connected: vec![],
        physics: None,
        transform: None,
        all_exported_vars: None,
    }
}

pub fn entity_with_state(
    path: &str,
    class: &str,
    pos: [f64; 3],
    groups: &[&str],
    state: &[(&str, serde_json::Value)],
) -> EntityData {
    let mut e = entity(path, class, pos, groups);
    for (k, v) in state {
        e.state.insert(k.to_string(), v.clone());
    }
    e
}

pub fn entity_2d(path: &str, class: &str, pos: [f64; 2], groups: &[&str]) -> EntityData {
    EntityData {
        path: path.into(),
        class: class.into(),
        position: pos.to_vec(),
        rotation_deg: vec![0.0],
        velocity: vec![0.0, 0.0],
        groups: groups.iter().map(|s| s.to_string()).collect(),
        visible: true,
        state: Default::default(),
        signals_recent: vec![],
        children: vec![],
        script: None,
        signals_connected: vec![],
        physics: None,
        transform: None,
        all_exported_vars: None,
    }
}
