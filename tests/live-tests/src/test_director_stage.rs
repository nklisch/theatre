use crate::dual_test;
use crate::harness::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

async fn director_creates_scene_stage_reads(b: &impl LiveBackend) {
    let scene_path = "tmp/live_test_create.tscn";
    // Godot derives root name from filename: LiveTestCreate
    let root_name = "LiveTestCreate";

    // Create scene
    b.director(
        "scene_create",
        json!({
            "scene_path": scene_path,
            "root_type": "Node3D"
        }),
    )
    .await
    .expect("scene_create")
    .unwrap_data();

    // Add a node (parent is the auto-derived root name)
    b.director(
        "node_add",
        json!({
            "scene_path": scene_path,
            "parent": root_name,
            "node_name": "Cube",
            "node_type": "MeshInstance3D"
        }),
    )
    .await
    .expect("node_add")
    .unwrap_data();

    // Read scene back
    let read = b
        .director("scene_read", json!({"scene_path": scene_path}))
        .await
        .expect("scene_read")
        .unwrap_data();

    // Verify Cube node exists in scene
    let has_cube = serde_json::to_string(&read)
        .map(|s| s.contains("Cube"))
        .unwrap_or(false);
    assert!(has_cube, "Scene should contain Cube node. Got: {read}");
}

async fn director_batch_builds_scene(b: &impl LiveBackend) {
    let scene_path = "tmp/live_test_batch.tscn";

    // Batch: create scene + add 3 nodes
    // Batch format: {"operation": "...", "params": {...}}
    // node_add uses node_type/node_name (not type/name)
    b.director(
        "batch",
        json!({
            "operations": [
                {"operation": "scene_create", "params": {
                    "scene_path": scene_path, "root_type": "Node3D"
                }},
                {"operation": "node_add", "params": {
                    "scene_path": scene_path,
                    "node_type": "StaticBody3D",
                    "node_name": "Floor"
                }},
                {"operation": "node_add", "params": {
                    "scene_path": scene_path,
                    "node_type": "CharacterBody3D",
                    "node_name": "Player"
                }},
                {"operation": "node_add", "params": {
                    "scene_path": scene_path,
                    "node_type": "CharacterBody3D",
                    "node_name": "Enemy"
                }}
            ]
        }),
    )
    .await
    .expect("batch")
    .unwrap_data();

    // Read scene back and verify all nodes
    let read = b
        .director("scene_read", json!({"scene_path": scene_path}))
        .await
        .expect("scene_read")
        .unwrap_data();

    let scene_str = serde_json::to_string(&read).unwrap_or_default();
    assert!(scene_str.contains("Floor"), "Scene should contain Floor: {read}");
    assert!(scene_str.contains("Player"), "Scene should contain Player: {read}");
    assert!(scene_str.contains("Enemy"), "Scene should contain Enemy: {read}");
}

async fn director_animation_roundtrip(b: &impl LiveBackend) {
    let anim_path = "tmp/live_test_walk.tres";

    // Create animation resource
    b.director(
        "animation_create",
        json!({
            "resource_path": anim_path,
            "length": 1.0,
            "loop_mode": "linear"
        }),
    )
    .await
    .expect("animation_create")
    .unwrap_data();

    // Add position track
    b.director(
        "animation_add_track",
        json!({
            "resource_path": anim_path,
            "track_type": "position_3d",
            "node_path": ".",
            "keyframes": [
                {"time": 0.0, "value": {"x": 0.0, "y": 0.0, "z": 0.0}},
                {"time": 1.0, "value": {"x": 5.0, "y": 0.0, "z": 0.0}}
            ]
        }),
    )
    .await
    .expect("animation_add_track")
    .unwrap_data();

    // Read animation back
    let read = b
        .director("animation_read", json!({"resource_path": anim_path}))
        .await
        .expect("animation_read")
        .unwrap_data();

    let length = read["length"].as_f64();
    assert!(
        length.is_some(),
        "Animation read should return length. Got: {read}"
    );
    assert!(
        (length.unwrap() - 1.0).abs() < 0.01,
        "Animation length should be 1.0, got: {:?}",
        length
    );

    let tracks = read["tracks"].as_array();
    assert!(
        tracks.is_some() && !tracks.unwrap().is_empty(),
        "Animation should have at least one track. Got: {read}"
    );
}

dual_test!(director_creates_scene_stage_reads, LIVE_3D, director_creates_scene_stage_reads);
dual_test!(director_batch_builds_scene, LIVE_3D, director_batch_builds_scene);
dual_test!(director_animation_roundtrip, LIVE_3D, director_animation_roundtrip);
