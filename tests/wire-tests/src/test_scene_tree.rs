/// Wire tests for `get_scene_tree`.
use crate::harness::GodotFixture;

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn scene_tree_roots_returns_at_least_one_node() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_scene_tree",
            serde_json::json!({ "action": "roots" }),
        )
        .unwrap()
        .unwrap_data();

    let nodes = data["roots"].as_array().expect("expected 'roots' array in response");
    assert!(!nodes.is_empty(), "expected at least one root node");
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

    let children = data["children"].as_array().expect("expected 'children' array in response");

    let names: Vec<&str> = children
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();

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

    // Result should include at least one match
    let results = data["results"].as_array().expect("expected 'results' array in response");
    assert!(!results.is_empty(), "expected at least one CharacterBody3D node in results");
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
