use serde_json::json;

use crate::harness::{CliFixture, DirectorFixture, OperationResultExt};

#[test]
#[ignore = "requires Godot binary"]
fn cli_scene_create_and_read() {
    let cli = CliFixture::new();
    let scene = DirectorFixture::temp_scene_path("cli_create_read");

    // Create via CLI
    let result = cli
        .run(
            "scene_create",
            json!({"scene_path": scene, "root_type": "Node2D"}),
        )
        .unwrap();
    let data = result.unwrap_data();
    assert_eq!(data["root_type"], "Node2D");

    // Read back via CLI
    let result = cli.run("scene_read", json!({"scene_path": scene})).unwrap();
    let data = result.unwrap_data();
    assert_eq!(data["root"]["type"], "Node2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn cli_node_add_and_read_back() {
    let cli = CliFixture::new();
    let scene = DirectorFixture::temp_scene_path("cli_node_add");

    cli.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    cli.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Sprite2D",
            "node_name": "Hero",
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = cli
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();

    let children = data["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "Hero");
    assert_eq!(children[0]["type"], "Sprite2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn cli_rejects_missing_project_path() {
    let cli = CliFixture::new();

    // Call with a non-existent project path
    let result = cli.run(
        "scene_read",
        json!({"project_path": "/nonexistent", "scene_path": "x.tscn"}),
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("project.godot"),
        "error should mention project.godot: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn cli_rejects_invalid_json() {
    let director_bin = crate::harness::workspace_binary("director")
        .canonicalize()
        .expect("director binary must be built");

    let output = std::process::Command::new(&director_bin)
        .args(["scene_read", "not valid json"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    // The CLI emits structured JSON errors on stdout (agents parse stdout);
    // stderr is reserved for diagnostics.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Invalid JSON"),
        "stdout should mention invalid JSON: {stdout}"
    );
}
