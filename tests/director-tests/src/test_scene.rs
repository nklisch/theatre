use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_create_then_read_round_trips() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("create_read");

    // Create
    let result = f
        .run(
            "scene_create",
            json!({
                "scene_path": scene,
                "root_type": "Node2D"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(result["root_type"], "Node2D");
    assert_eq!(result["path"], scene);

    // Read back
    let result = f
        .run(
            "scene_read",
            json!({
                "scene_path": scene
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(result["root"]["type"], "Node2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_create_invalid_type_returns_error() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "scene_create",
            json!({
                "scene_path": DirectorFixture::temp_scene_path("invalid_type"),
                "root_type": "NotARealClass"
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("Unknown node type"));
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_read_nonexistent_returns_error() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "scene_read",
            json!({
                "scene_path": "nonexistent/missing.tscn"
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_read_with_depth_limit() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("depth_limit");

    // Create scene with nested nodes
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Child"}),
    )
    .unwrap();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "Child", "node_type": "Sprite2D", "node_name": "Grandchild"})).unwrap();

    // Read with depth=1 — should include root + direct children, not grandchildren
    let result = f
        .run("scene_read", json!({"scene_path": scene, "depth": 1}))
        .unwrap()
        .unwrap_data();
    let root = &result["root"];
    assert!(root["children"].as_array().is_some());
    let child = &root["children"][0];
    assert!(child.get("children").is_none() || child["children"].as_array().unwrap().is_empty());
}
