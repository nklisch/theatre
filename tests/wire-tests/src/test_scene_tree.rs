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

    // Response is wrapped in "data" field
    let nodes = data["data"].as_array().unwrap_or_else(|| {
        data.as_array().expect("expected array response for roots")
    });
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
                "node": "TestScene3D/Enemies"
            }),
        )
        .unwrap()
        .unwrap_data();

    // Find children array (may be nested under "data")
    let children = if let Some(arr) = data["data"].as_array() {
        arr
    } else {
        data.as_array().expect("expected array")
    };

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
    assert!(data.get("data").is_some() || data.is_array(), "expected search result");
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
                "node": "TestScene3D/DoesNotExist"
            }),
        )
        .unwrap();

    assert!(result.is_err(), "expected error for missing node");
}
