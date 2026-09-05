/// Wire tests for `get_scene_tree`.
use crate::harness::GodotFixture;

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn scene_tree_roots_returns_at_least_one_node() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    f.query(
        "execute_action",
        serde_json::json!({
            "action":"spawn_node", "scene_path":"res://spawn_test.tscn",
            "parent":"/root", "name":"DiscoverySibling"
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .query("get_scene_tree", serde_json::json!({ "action": "roots" }))
        .unwrap()
        .unwrap_data();

    let nodes = data["roots"]
        .as_array()
        .expect("expected 'roots' array in response");
    assert!(
        nodes
            .iter()
            .any(|node| node["path"] == "/root/DiscoverySibling")
    );
    for node in nodes {
        let path = node["path"].as_str().expect("reusable root path");
        assert!(path.starts_with("/root/"));
        let children = f
            .query(
                "get_scene_tree",
                serde_json::json!({
                    "action":"children", "node":path
                }),
            )
            .unwrap()
            .unwrap_data();
        for child in children["children"].as_array().unwrap() {
            let path = child["path"].as_str().expect("reusable child path");
            assert!(path.starts_with("/root/"));
            f.query(
                "get_node_inspect",
                serde_json::json!({
                    "path":path, "include":["transform"]
                }),
            )
            .unwrap()
            .unwrap_data();
        }
    }
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn scene_tree_children_returns_expected_nodes() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_scene_tree",
            serde_json::json!({
                "action": "children",
                "node": "Enemies"
            }),
        )
        .unwrap()
        .unwrap_data();

    let children = data["children"]
        .as_array()
        .expect("expected 'children' array in response");

    let names: Vec<&str> = children.iter().filter_map(|c| c["name"].as_str()).collect();

    assert!(
        names.contains(&"Scout"),
        "expected Scout in Enemies children, got: {names:?}"
    );
    assert!(
        names.contains(&"Tank"),
        "expected Tank in Enemies children, got: {names:?}"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn scene_tree_find_by_class() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_scene_tree",
            serde_json::json!({
                "action": "find",
                "find_by": "class",
                "find_value": "CharacterBody3D"
            }),
        )
        .unwrap()
        .unwrap_data();

    // Find uses the same reusable absolute path contract as root/child listings.
    let results = data["results"]
        .as_array()
        .expect("expected 'results' array in response");
    assert!(
        !results.is_empty(),
        "expected at least one CharacterBody3D node in results"
    );
    for node in results {
        let path = node["path"].as_str().unwrap();
        assert!(path.starts_with("/root/"));
        f.query(
            "get_scene_tree",
            serde_json::json!({"action":"children", "node":path}),
        )
        .unwrap()
        .unwrap_data();
    }
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn scene_tree_missing_node_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "get_scene_tree",
            serde_json::json!({
                "action": "children",
                "node": "DoesNotExist"
            }),
        )
        .unwrap();

    assert!(result.is_err(), "expected error for missing node");
}
