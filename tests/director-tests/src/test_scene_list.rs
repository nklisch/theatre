use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_returns_project_scenes() {
    // The test project has test_scene_2d.tscn and test_scene_3d.tscn at root
    let f = DirectorFixture::new();
    let data = f
        .run("scene_list", json!({"directory": ""}))
        .unwrap()
        .unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    assert!(
        scenes.len() >= 2,
        "expected at least 2 scenes, got {}",
        scenes.len()
    );

    // Verify structure of entries
    let first = &scenes[0];
    assert!(first["path"].is_string());
    assert!(first["root_type"].is_string());
    assert!(first["node_count"].is_number());
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_with_directory_filter() {
    let f = DirectorFixture::new();

    // Create a scene in a subdirectory
    let scene = "tmp/subdir/listed.tscn";
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node3D"}),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run("scene_list", json!({"directory": "tmp/subdir"}))
        .unwrap()
        .unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes[0]["path"], "tmp/subdir/listed.tscn");
    assert_eq!(scenes[0]["root_type"], "Node3D");
    assert_eq!(scenes[0]["node_count"], 1);
}

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_nonexistent_directory_returns_error() {
    let f = DirectorFixture::new();
    let err = f
        .run("scene_list", json!({"directory": "nonexistent/path"}))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not found") || err.contains("does not exist"));
}
