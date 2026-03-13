use crate::harness::{DirectorFixture, assert_approx, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn node_add_to_root() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("node_add_root");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap();

    let result = f
        .run(
            "node_add",
            json!({
                "scene_path": scene,
                "node_type": "Sprite2D",
                "node_name": "MySprite"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["type"], "Sprite2D");
    assert_eq!(result["node_path"], "MySprite");
}

#[test]
#[ignore = "requires Godot binary"]
fn node_add_with_properties() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("node_add_props");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Sprite2D",
            "node_name": "S",
            "properties": {"position": {"x": 100, "y": 200}, "visible": false}
        }),
    )
    .unwrap()
    .unwrap_data();

    // Verify via scene_read
    let data = f
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let sprite = &data["root"]["children"][0];
    assert_eq!(sprite["name"], "S");
    assert_approx(
        sprite["properties"]["position"]["x"].as_f64().unwrap(),
        100.0,
    );
    assert_approx(
        sprite["properties"]["position"]["y"].as_f64().unwrap(),
        200.0,
    );
    assert_eq!(sprite["properties"]["visible"], false);
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_properties_vector2() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("set_props_v2");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Sprite2D", "node_name": "S"}),
    )
    .unwrap();

    let result = f
        .run(
            "node_set_properties",
            json!({
                "scene_path": scene,
                "node_path": "S",
                "properties": {"position": {"x": 50, "y": 75}}
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(
        result["properties_set"]
            .as_array()
            .unwrap()
            .contains(&json!("position"))
    );

    // Verify
    let data = f
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let pos = &data["root"]["children"][0]["properties"]["position"];
    assert_approx(pos["x"].as_f64().unwrap(), 50.0);
    assert_approx(pos["y"].as_f64().unwrap(), 75.0);
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_properties_unknown_property_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("set_props_unknown");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Sprite2D", "node_name": "S"}),
    )
    .unwrap();

    let err = f
        .run(
            "node_set_properties",
            json!({
                "scene_path": scene,
                "node_path": "S",
                "properties": {"nonexistent_property": 42}
            }),
        )
        .unwrap()
        .unwrap_err();

    assert!(err.contains("Unknown property"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_remove_with_children() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("node_remove");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Parent"}),
    )
    .unwrap();
    f.run("node_add", json!({"scene_path": scene, "parent_path": "Parent", "node_type": "Sprite2D", "node_name": "Child"})).unwrap();

    let result = f
        .run(
            "node_remove",
            json!({
                "scene_path": scene,
                "node_path": "Parent"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["removed"], "Parent");
    assert_eq!(result["children_removed"], 1);

    // Verify parent is gone
    let data = f
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    assert!(
        data["root"].get("children").is_none()
            || data["root"]["children"].as_array().unwrap().is_empty()
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn node_remove_root_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("remove_root");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap();

    let err = f
        .run(
            "node_remove",
            json!({
                "scene_path": scene,
                "node_path": "."
            }),
        )
        .unwrap()
        .unwrap_err();

    assert!(err.contains("root"));
}
