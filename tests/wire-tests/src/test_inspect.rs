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
                "path": "Player",
                "include": ["transform"]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(
        data.get("transform").is_some(),
        "transform category missing"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_returns_state_category() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "Enemies/Scout",
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
                "path": "Player",
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
                "path": "DoesNotExist",
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
                "path": "Player",
                "include": ["transform"]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(data.get("path").is_some(), "path field missing");
    assert!(data.get("class").is_some(), "class field missing");
}

// --- resources category ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_resources_not_returned_by_default() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // No explicit include → uses server default (all categories except resources)
    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "Player",
                "include": ["transform", "state", "children"]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(
        data.get("resources").is_none(),
        "resources should be absent when not in include list"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_resources_returns_collision_shapes() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Player has one CollisionShape3D child with a CapsuleShape3D
    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "Player",
                "include": ["resources"]
            }),
        )
        .unwrap()
        .unwrap_data();

    let resources = data.get("resources").expect("resources key missing");
    let shapes = resources["collision_shapes"]
        .as_array()
        .expect("collision_shapes should be an array");
    assert_eq!(
        shapes.len(),
        1,
        "Player should have exactly one collision shape"
    );

    let shape = &shapes[0];
    assert_eq!(
        shape["type"].as_str().unwrap_or(""),
        "CapsuleShape3D",
        "expected CapsuleShape3D"
    );
    assert!(
        !shape["disabled"].as_bool().unwrap_or(true),
        "shape should not be disabled"
    );
    assert!(
        shape["inline"].as_bool().unwrap_or(false),
        "inline capsule shape should be inline"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_resources_capsule_shape_3d_dimensions() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // CapsuleShape3D_player: radius=0.4, height=1.8
    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "Player",
                "include": ["resources"]
            }),
        )
        .unwrap()
        .unwrap_data();

    let shape = &data["resources"]["collision_shapes"][0];
    let dims = shape["dimensions"]
        .as_object()
        .expect("dimensions map missing");

    let radius = dims["radius"].as_f64().expect("radius missing");
    let height = dims["height"].as_f64().expect("height missing");

    assert!(
        (radius - 0.4).abs() < 0.01,
        "radius should be ~0.4, got {radius}"
    );
    assert!(
        (height - 1.8).abs() < 0.01,
        "height should be ~1.8, got {height}"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_resources_box_shape_3d_dimensions() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Floor has BoxShape3D: size=Vector3(20, 0.2, 20)
    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "Floor",
                "include": ["resources"]
            }),
        )
        .unwrap()
        .unwrap_data();

    let resources = data.get("resources").expect("resources key missing");
    let shapes = resources["collision_shapes"]
        .as_array()
        .expect("collision_shapes should be an array");
    assert!(!shapes.is_empty(), "Floor should have a collision shape");

    let shape = &shapes[0];
    assert_eq!(
        shape["type"].as_str().unwrap_or(""),
        "BoxShape3D",
        "expected BoxShape3D"
    );

    let dims = shape["dimensions"]
        .as_object()
        .expect("dimensions map missing");
    let size = dims["size"].as_array().expect("size should be array");
    assert_eq!(size.len(), 3, "BoxShape3D size should have 3 components");
    assert!(
        (size[0].as_f64().unwrap_or(0.0) - 20.0).abs() < 0.1,
        "size.x should be ~20, got {:?}",
        size[0]
    );
    assert!(
        (size[2].as_f64().unwrap_or(0.0) - 20.0).abs() < 0.1,
        "size.z should be ~20, got {:?}",
        size[2]
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inspect_resources_2d_collision_shape_dimensions() {
    let mut f = GodotFixture::start("test_scene_2d.tscn").unwrap();

    // Player has CapsuleShape2D: radius=16.0, height=48.0
    let data = f
        .query(
            "get_node_inspect",
            serde_json::json!({
                "path": "Player",
                "include": ["resources"]
            }),
        )
        .unwrap()
        .unwrap_data();

    let resources = data.get("resources").expect("resources key missing");
    let shapes = resources["collision_shapes"]
        .as_array()
        .expect("collision_shapes should be an array");
    assert_eq!(shapes.len(), 1, "2D Player should have one collision shape");

    let shape = &shapes[0];
    assert_eq!(
        shape["type"].as_str().unwrap_or(""),
        "CapsuleShape2D",
        "expected CapsuleShape2D"
    );

    let dims = shape["dimensions"]
        .as_object()
        .expect("dimensions map missing");
    let radius = dims["radius"].as_f64().expect("radius missing");
    let height = dims["height"].as_f64().expect("height missing");

    assert!(
        (radius - 16.0).abs() < 0.5,
        "radius should be ~16, got {radius}"
    );
    assert!(
        (height - 48.0).abs() < 0.5,
        "height should be ~48, got {height}"
    );
}
