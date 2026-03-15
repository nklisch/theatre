use crate::dual_test;
use crate::harness::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

async fn director_creates_scene_stage_reads(b: &impl LiveBackend) {
    let scene_path = "tmp/live_test_create.tscn";

    // Create scene
    b.director(
        "scene_create",
        json!({
            "path": scene_path,
            "root_name": "TestRoot",
            "root_type": "Node3D"
        }),
    )
    .await
    .expect("scene_create")
    .unwrap_data();

    // Add a node
    b.director(
        "node_add",
        json!({
            "scene_path": scene_path,
            "parent": "TestRoot",
            "name": "Cube",
            "type": "MeshInstance3D"
        }),
    )
    .await
    .expect("node_add")
    .unwrap_data();

    // Read scene back
    let read = b
        .director("scene_read", json!({"path": scene_path}))
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
    b.director(
        "batch",
        json!({
            "operations": [
                {
                    "op": "scene_create",
                    "path": scene_path,
                    "root_name": "BatchRoot",
                    "root_type": "Node3D"
                },
                {
                    "op": "node_add",
                    "scene_path": scene_path,
                    "parent": "BatchRoot",
                    "name": "Floor",
                    "type": "StaticBody3D"
                },
                {
                    "op": "node_add",
                    "scene_path": scene_path,
                    "parent": "BatchRoot",
                    "name": "Player",
                    "type": "CharacterBody3D"
                },
                {
                    "op": "node_add",
                    "scene_path": scene_path,
                    "parent": "BatchRoot",
                    "name": "Enemy",
                    "type": "CharacterBody3D"
                }
            ]
        }),
    )
    .await
    .expect("batch")
    .unwrap_data();

    // Read scene back and verify all nodes
    let read = b
        .director("scene_read", json!({"path": scene_path}))
        .await
        .expect("scene_read")
        .unwrap_data();

    let scene_str = serde_json::to_string(&read).unwrap_or_default();
    assert!(scene_str.contains("Floor"), "Scene should contain Floor: {read}");
    assert!(scene_str.contains("Player"), "Scene should contain Player: {read}");
    assert!(scene_str.contains("Enemy"), "Scene should contain Enemy: {read}");
}

async fn director_animation_roundtrip(b: &impl LiveBackend) {
    let scene_path = "tmp/live_test_anim.tscn";

    // Create scene with AnimationPlayer
    b.director(
        "scene_create",
        json!({
            "path": scene_path,
            "root_name": "AnimRoot",
            "root_type": "Node3D"
        }),
    )
    .await
    .expect("scene_create")
    .unwrap_data();

    b.director(
        "node_add",
        json!({
            "scene_path": scene_path,
            "parent": "AnimRoot",
            "name": "AnimPlayer",
            "type": "AnimationPlayer"
        }),
    )
    .await
    .expect("node_add")
    .unwrap_data();

    // Create animation
    b.director(
        "animation_create",
        json!({
            "scene_path": scene_path,
            "player_path": "AnimRoot/AnimPlayer",
            "animation_name": "walk",
            "length": 1.0,
            "loop_mode": "linear"
        }),
    )
    .await
    .expect("animation_create")
    .unwrap_data();

    // Add track
    b.director(
        "animation_add_track",
        json!({
            "scene_path": scene_path,
            "player_path": "AnimRoot/AnimPlayer",
            "animation_name": "walk",
            "type": "position_3d",
            "node_path": ".",
            "keyframes": [
                {"time": 0.0, "value": [0.0, 0.0, 0.0]},
                {"time": 1.0, "value": [5.0, 0.0, 0.0]}
            ]
        }),
    )
    .await
    .expect("animation_add_track")
    .unwrap_data();

    // Read animation back
    let read = b
        .director(
            "animation_read",
            json!({
                "scene_path": scene_path,
                "player_path": "AnimRoot/AnimPlayer",
                "animation_name": "walk"
            }),
        )
        .await
        .expect("animation_read")
        .unwrap_data();

    let anim_str = serde_json::to_string(&read).unwrap_or_default();
    assert!(
        anim_str.contains("walk") || read["length"].as_f64().is_some(),
        "Animation read should return animation data. Got: {read}"
    );
}

dual_test!(director_creates_scene_stage_reads, LIVE_3D, director_creates_scene_stage_reads);
dual_test!(director_batch_builds_scene, LIVE_3D, director_batch_builds_scene);
dual_test!(director_animation_roundtrip, LIVE_3D, director_animation_roundtrip);
