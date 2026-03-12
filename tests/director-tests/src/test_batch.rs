use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn batch_creates_scene_and_adds_nodes() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("batch_basic");

    let data = f
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "scene_create", "params": {
                        "scene_path": scene, "root_type": "Node2D"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene, "node_type": "Sprite2D",
                        "node_name": "Hero"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene, "node_type": "CollisionShape2D",
                        "node_name": "Hitbox"
                    }},
                ]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["completed"], 3);
    assert_eq!(data["failed"], 0);
    assert_eq!(data["results"].as_array().unwrap().len(), 3);

    // Verify the scene was actually created with both nodes
    let tree = f
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let children = tree["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn batch_stop_on_error_true() {
    let f = DirectorFixture::new();

    let data = f
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "scene_read", "params": {
                        "scene_path": "nonexistent.tscn"
                    }},
                    {"operation": "scene_create", "params": {
                        "scene_path": "tmp/should_not_run.tscn",
                        "root_type": "Node2D"
                    }},
                ],
                "stop_on_error": true
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["completed"], 0);
    assert_eq!(data["failed"], 1);
    // Second operation should NOT have run
    assert_eq!(data["results"].as_array().unwrap().len(), 1);
}

#[test]
#[ignore = "requires Godot binary"]
fn batch_stop_on_error_false_continues() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("batch_continue");

    let data = f
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "scene_read", "params": {
                        "scene_path": "nonexistent.tscn"
                    }},
                    {"operation": "scene_create", "params": {
                        "scene_path": scene, "root_type": "Node2D"
                    }},
                ],
                "stop_on_error": false
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["completed"], 1);
    assert_eq!(data["failed"], 1);
    assert_eq!(data["results"].as_array().unwrap().len(), 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn batch_rejects_nested_batch() {
    let f = DirectorFixture::new();

    let data = f
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "batch", "params": {"operations": []}},
                ]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["failed"], 1);
    let err = &data["results"][0];
    assert_eq!(err["success"], false);
    assert!(err["error"].as_str().unwrap().contains("nested"));
}

#[test]
#[ignore = "requires Godot binary"]
fn batch_empty_operations_errors() {
    let f = DirectorFixture::new();
    let err = f
        .run("batch", json!({"operations": []}))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("empty"));
}
