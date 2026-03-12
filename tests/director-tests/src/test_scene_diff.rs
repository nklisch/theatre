use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_identical_scenes_no_changes() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("diff_identical");
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run("scene_diff", json!({"scene_a": scene, "scene_b": scene}))
        .unwrap()
        .unwrap_data();

    assert!(data["added"].as_array().unwrap().is_empty());
    assert!(data["removed"].as_array().unwrap().is_empty());
    assert!(data["changed"].as_array().unwrap().is_empty());
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_detects_added_node() {
    let f = DirectorFixture::new();
    let scene_a = DirectorFixture::temp_scene_path("diff_a_added");
    let scene_b = DirectorFixture::temp_scene_path("diff_b_added");

    // Scene A: just root
    f.run(
        "scene_create",
        json!({"scene_path": scene_a, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // Scene B: root + child
    f.run(
        "scene_create",
        json!({"scene_path": scene_b, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene_b, "node_type": "Sprite2D", "node_name": "NewSprite"}),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "scene_diff",
            json!({"scene_a": scene_a, "scene_b": scene_b}),
        )
        .unwrap()
        .unwrap_data();

    let added = data["added"].as_array().unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0]["type"], "Sprite2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_detects_removed_node() {
    let f = DirectorFixture::new();
    let scene_a = DirectorFixture::temp_scene_path("diff_a_removed");
    let scene_b = DirectorFixture::temp_scene_path("diff_b_removed");

    // Scene A: root + child
    f.run(
        "scene_create",
        json!({"scene_path": scene_a, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene_a, "node_type": "Sprite2D", "node_name": "OldSprite"}),
    )
    .unwrap()
    .unwrap_data();

    // Scene B: just root
    f.run(
        "scene_create",
        json!({"scene_path": scene_b, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "scene_diff",
            json!({"scene_a": scene_a, "scene_b": scene_b}),
        )
        .unwrap()
        .unwrap_data();

    let removed = data["removed"].as_array().unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0]["type"], "Sprite2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_detects_property_change() {
    let f = DirectorFixture::new();
    let scene_a = DirectorFixture::temp_scene_path("diff_a_props");
    let scene_b = DirectorFixture::temp_scene_path("diff_b_props");

    // Scene A: Sprite at default position
    f.run(
        "scene_create",
        json!({"scene_path": scene_a, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene_a, "node_type": "Sprite2D", "node_name": "Sprite"}),
    )
    .unwrap()
    .unwrap_data();

    // Scene B: same structure, Sprite at (100,200)
    f.run(
        "scene_create",
        json!({"scene_path": scene_b, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene_b, "node_type": "Sprite2D", "node_name": "Sprite"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_set_properties",
        json!({
            "scene_path": scene_b,
            "node_path": "Sprite",
            "properties": {"position": {"x": 100, "y": 200}}
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "scene_diff",
            json!({"scene_a": scene_a, "scene_b": scene_b}),
        )
        .unwrap()
        .unwrap_data();

    let changed = data["changed"].as_array().unwrap();
    assert!(!changed.is_empty());
    let pos_change = changed.iter().find(|c| c["property"] == "position");
    assert!(pos_change.is_some());
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_nonexistent_scene_errors() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "scene_diff",
            json!({"scene_a": "nonexistent_a.tscn", "scene_b": "nonexistent_b.tscn"}),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not found"));
}
