use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_sets_collision_layer() {
    let f = DirectorFixture::new();
    // Create scene with CharacterBody2D
    let scene_path = "tmp/test_physics_layer.tscn";
    f.run(
        "scene_create",
        json!({
            "scene_path": scene_path,
            "root_type": "CharacterBody2D",
            "root_name": "Player",
        }),
    )
    .unwrap()
    .unwrap_data();

    // Set collision_layer = 5
    let data = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene_path,
                "node_path": ".",
                "collision_layer": 5,
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["collision_layer"], 5);
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_sets_collision_mask() {
    let f = DirectorFixture::new();
    let scene_path = "tmp/test_physics_mask.tscn";
    f.run(
        "scene_create",
        json!({
            "scene_path": scene_path,
            "root_type": "Area3D",
            "root_name": "Area",
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene_path,
                "node_path": ".",
                "collision_mask": 0xFF_u32,
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["collision_mask"], 255);
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_sets_both() {
    let f = DirectorFixture::new();
    let scene_path = "tmp/test_physics_both.tscn";
    f.run(
        "scene_create",
        json!({
            "scene_path": scene_path,
            "root_type": "StaticBody2D",
            "root_name": "Wall",
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene_path,
                "node_path": ".",
                "collision_layer": 3,
                "collision_mask": 7,
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["collision_layer"], 3);
    assert_eq!(data["collision_mask"], 7);
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_rejects_non_physics_node() {
    let f = DirectorFixture::new();
    let scene_path = "tmp/test_physics_non_physics.tscn";
    f.run(
        "scene_create",
        json!({
            "scene_path": scene_path,
            "root_type": "Node2D",
            "root_name": "Root",
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene_path,
                "node_path": ".",
                "collision_layer": 1,
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("collision_layer") || err.contains("collision properties"),
        "expected collision property error, got: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layers_rejects_neither_set() {
    let f = DirectorFixture::new();
    let scene_path = "tmp/test_physics_neither.tscn";
    f.run(
        "scene_create",
        json!({
            "scene_path": scene_path,
            "root_type": "CharacterBody2D",
            "root_name": "Player",
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene_path,
                "node_path": ".",
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("collision_layer") || err.contains("collision_mask"),
        "expected validation error, got: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layer_names_writes_project_settings() {
    let f = DirectorFixture::new();
    let data = f
        .run(
            "physics_set_layer_names",
            json!({
                "layer_type": "2d_physics",
                "layers": {
                    "1": "player",
                    "2": "enemies",
                },
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["layer_type"], "2d_physics");
    assert_eq!(data["layers_set"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layer_names_rejects_invalid_type() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "physics_set_layer_names",
            json!({
                "layer_type": "invalid",
                "layers": {"1": "test"},
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("Invalid layer_type"),
        "expected invalid type error, got: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn physics_set_layer_names_rejects_out_of_range() {
    let f = DirectorFixture::new();
    // Layer 0 is out of range
    let err = f
        .run(
            "physics_set_layer_names",
            json!({
                "layer_type": "2d_physics",
                "layers": {"0": "zero"},
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("1-32") || err.contains("Layer number"),
        "expected range error, got: {err}"
    );

    // Layer 33 is out of range
    let err2 = f
        .run(
            "physics_set_layer_names",
            json!({
                "layer_type": "2d_physics",
                "layers": {"33": "overflow"},
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err2.contains("1-32") || err2.contains("Layer number"),
        "expected range error, got: {err2}"
    );
}
