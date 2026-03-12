use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_with_pattern_filter() {
    let f = DirectorFixture::new();

    // Create scenes in different dirs
    f.run(
        "scene_create",
        json!({"scene_path": "tmp/deferred/a.tscn", "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "scene_create",
        json!({"scene_path": "tmp/deferred/b.tscn", "root_type": "Node3D"}),
    )
    .unwrap();
    f.run(
        "scene_create",
        json!({"scene_path": "tmp/other/c.tscn", "root_type": "Node2D"}),
    )
    .unwrap();

    let data = f
        .run("scene_list", json!({"pattern": "tmp/deferred/*.tscn"}))
        .unwrap()
        .unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    assert_eq!(scenes.len(), 2);
    assert!(scenes
        .iter()
        .all(|s| s["path"].as_str().unwrap().starts_with("tmp/deferred/")));
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_no_pattern_returns_all() {
    let f = DirectorFixture::new();

    f.run(
        "scene_create",
        json!({"scene_path": "tmp/pattern_test/x.tscn", "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "scene_create",
        json!({"scene_path": "tmp/pattern_test/y.tscn", "root_type": "Node2D"}),
    )
    .unwrap();

    // Without pattern, should return all scenes (backward-compatible)
    let data = f.run("scene_list", json!({})).unwrap().unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    assert!(scenes.len() >= 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_depth_parameter() {
    let f = DirectorFixture::new();

    // Create a material resource to read
    f.run(
        "material_create",
        json!({
            "resource_path": "tmp/test_material.tres",
            "material_type": "StandardMaterial3D",
        }),
    )
    .unwrap();

    // Read at depth 0 — nested resources returned as paths
    let data0 = f
        .run(
            "resource_read",
            json!({
                "resource_path": "tmp/test_material.tres",
                "depth": 0,
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data0["type"], "StandardMaterial3D");

    // Read at depth 1 (default) — same shape as depth 0 for a simple material
    let data1 = f
        .run(
            "resource_read",
            json!({
                "resource_path": "tmp/test_material.tres",
                "depth": 1,
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data1["type"], "StandardMaterial3D");
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_same_scene_no_changes() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("diff_same");

    f.run("scene_create", json!({"scene_path": &scene, "root_type": "Node2D"}))
        .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "Child"}),
    )
    .unwrap();

    // Comparing the same scene with itself should return no changes
    let data = f
        .run(
            "scene_diff",
            json!({"scene_a": &scene, "scene_b": &scene}),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["added"].as_array().unwrap().len(), 0);
    assert_eq!(data["removed"].as_array().unwrap().len(), 0);
    assert_eq!(data["changed"].as_array().unwrap().len(), 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_git_ref_invalid_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("diff_git_err");

    f.run("scene_create", json!({"scene_path": &scene, "root_type": "Node2D"}))
        .unwrap();

    // Using a clearly invalid git ref should return an error
    let result = f
        .run(
            "scene_diff",
            json!({"scene_a": &scene, "scene_b": "INVALID_REF_XXXX:nonexistent.tscn"}),
        )
        .unwrap();

    assert!(!result.success);
}
