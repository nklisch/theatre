use serde_json::json;

use crate::harness::{DirectorFixture, EditorFixture};

// Each test uses a distinct port (16561–16565) to avoid conflicts with
// daemon tests (16551–16552) and other parallel tests.

#[test]
#[ignore = "requires Godot binary"]
fn editor_fixture_creates_and_reads_scene() {
    let mut e = EditorFixture::start_with_port(16561);
    let scene_path = DirectorFixture::temp_scene_path("editor_create");

    let result = e
        .run("scene_create", json!({
            "scene_path": scene_path,
            "root_type": "Node2D",
        }))
        .unwrap();
    result.unwrap_data();

    let read_result = e
        .run("scene_read", json!({
            "scene_path": scene_path,
        }))
        .unwrap();
    let data = read_result.unwrap_data();
    assert_eq!(data["root"]["type"], "Node2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_fixture_node_add_and_set_properties() {
    let mut e = EditorFixture::start_with_port(16562);
    let scene_path = DirectorFixture::temp_scene_path("editor_nodeadd");

    e.run("scene_create", json!({
        "scene_path": scene_path,
        "root_type": "Node2D",
    }))
    .unwrap()
    .unwrap_data();

    e.run("node_add", json!({
        "scene_path": scene_path,
        "node_type": "Sprite2D",
        "node_name": "TestSprite",
    }))
    .unwrap()
    .unwrap_data();

    let read = e
        .run("scene_read", json!({
            "scene_path": scene_path,
        }))
        .unwrap()
        .unwrap_data();

    let children = read["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "TestSprite");
    assert_eq!(children[0]["type"], "Sprite2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_ping_returns_editor_backend() {
    let mut e = EditorFixture::start_with_port(16563);
    let result = e.run("ping", json!({})).unwrap().unwrap_data();
    assert_eq!(result["backend"], "editor");
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_fixture_node_remove() {
    let mut e = EditorFixture::start_with_port(16564);
    let scene_path = DirectorFixture::temp_scene_path("editor_noderemove");

    e.run("scene_create", json!({
        "scene_path": scene_path,
        "root_type": "Node2D",
    }))
    .unwrap()
    .unwrap_data();

    e.run("node_add", json!({
        "scene_path": scene_path,
        "node_type": "Node2D",
        "node_name": "Child",
    }))
    .unwrap()
    .unwrap_data();

    let remove = e
        .run("node_remove", json!({
            "scene_path": scene_path,
            "node_path": "Child",
        }))
        .unwrap()
        .unwrap_data();

    assert_eq!(remove["removed"], "Child");

    let read = e
        .run("scene_read", json!({
            "scene_path": scene_path,
        }))
        .unwrap()
        .unwrap_data();

    assert!(
        read["root"]["children"].is_null()
            || read["root"]["children"]
                .as_array()
                .map(|a| a.is_empty())
                .unwrap_or(true)
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_fixture_scene_list() {
    let mut e = EditorFixture::start_with_port(16565);
    let scene_path = DirectorFixture::temp_scene_path("editor_list");

    e.run("scene_create", json!({
        "scene_path": scene_path,
        "root_type": "Node",
    }))
    .unwrap()
    .unwrap_data();

    let data = e
        .run("scene_list", json!({}))
        .unwrap()
        .unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    assert!(scenes.iter().any(|s| s["path"] == scene_path));
}
