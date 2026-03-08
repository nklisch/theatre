/// Wire tests for `get_node_inspect`.
use crate::harness::GodotFixture;

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_returns_transform_category() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "TestScene3D/Player",
                "include": ["transform"]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(data.get("transform").is_some(), "transform category missing");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_returns_state_category() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "TestScene3D/Enemies/Scout",
                "include": ["state"]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(data.get("state").is_some(), "state category missing");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_returns_children_category() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "TestScene3D/Player",
                "include": ["children"]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(data.get("children").is_some(), "children category missing");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_missing_node_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "TestScene3D/DoesNotExist",
                "include": ["transform"]
            }),
        )
        .unwrap();

    assert!(result.is_err(), "expected error for missing node");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_returns_path_and_class() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "TestScene3D/Player",
                "include": ["transform"]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(data.get("path").is_some(), "path field missing");
    assert!(data.get("class").is_some(), "class field missing");
}
