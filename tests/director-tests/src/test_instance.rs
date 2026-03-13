use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_basic() {
    let f = DirectorFixture::new();

    // Create the scene to be instanced
    let child_scene = DirectorFixture::temp_scene_path("instance_child");
    f.run(
        "scene_create",
        json!({"scene_path": child_scene, "root_type": "CharacterBody2D"}),
    )
    .unwrap()
    .unwrap_data();

    // Add a node to the child scene so we can verify it appears
    f.run(
        "node_add",
        json!({"scene_path": child_scene, "node_type": "Sprite2D", "node_name": "Sprite"}),
    )
    .unwrap()
    .unwrap_data();

    // Create the parent scene
    let parent_scene = DirectorFixture::temp_scene_path("instance_parent");
    f.run(
        "scene_create",
        json!({"scene_path": parent_scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // Instance the child into the parent
    let data = f
        .run(
            "scene_add_instance",
            json!({"scene_path": parent_scene, "instance_scene": child_scene}),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["instance_scene"], child_scene);
    assert!(data["node_path"].is_string());

    // Read back and verify the instance appears with its children
    let tree = f
        .run("scene_read", json!({"scene_path": parent_scene}))
        .unwrap()
        .unwrap_data();

    let root = &tree["root"];
    let children = root["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["type"], "CharacterBody2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_with_custom_name() {
    let f = DirectorFixture::new();

    let child_scene = DirectorFixture::temp_scene_path("instance_named_child");
    f.run(
        "scene_create",
        json!({"scene_path": child_scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    let parent_scene = DirectorFixture::temp_scene_path("instance_named_parent");
    f.run(
        "scene_create",
        json!({"scene_path": parent_scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "scene_add_instance",
        json!({"scene_path": parent_scene, "instance_scene": child_scene, "node_name": "MyPlayer"}),
    )
    .unwrap()
    .unwrap_data();

    // Read back and check name
    let tree = f
        .run("scene_read", json!({"scene_path": parent_scene}))
        .unwrap()
        .unwrap_data();

    assert_eq!(tree["root"]["children"][0]["name"], "MyPlayer");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_missing_scene_returns_error() {
    let f = DirectorFixture::new();

    let parent_scene = DirectorFixture::temp_scene_path("instance_err_parent");
    f.run(
        "scene_create",
        json!({"scene_path": parent_scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "scene_add_instance",
            json!({"scene_path": parent_scene, "instance_scene": "nonexistent/nope.tscn"}),
        )
        .unwrap()
        .unwrap_err();

    assert!(err.contains("not found") || err.contains("does not exist"));
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_add_instance_name_collision_returns_error() {
    let f = DirectorFixture::new();

    let child_scene = DirectorFixture::temp_scene_path("instance_collision_child");
    f.run(
        "scene_create",
        json!({"scene_path": child_scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    let parent_scene = DirectorFixture::temp_scene_path("instance_collision_parent");
    f.run(
        "scene_create",
        json!({"scene_path": parent_scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // Add first instance
    f.run(
        "scene_add_instance",
        json!({"scene_path": parent_scene, "instance_scene": child_scene}),
    )
    .unwrap()
    .unwrap_data();

    // Second instance with same name should error
    let err = f
        .run(
            "scene_add_instance",
            json!({"scene_path": parent_scene, "instance_scene": child_scene}),
        )
        .unwrap()
        .unwrap_err();

    assert!(err.to_lowercase().contains("name") || err.to_lowercase().contains("already exists"));
}
