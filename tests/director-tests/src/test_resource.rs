use crate::harness::{assert_approx, DirectorFixture};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_tres_material() {
    let f = DirectorFixture::new();
    let data = f
        .run(
            "resource_read",
            json!({"resource_path": "fixtures/test_material.tres"}),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["type"], "StandardMaterial3D");
    assert_eq!(data["path"], "fixtures/test_material.tres");

    let props = &data["properties"];
    // albedo_color = Color(1, 0, 0, 1)
    assert_approx(props["albedo_color"]["r"].as_f64().unwrap(), 1.0);
    assert_approx(props["albedo_color"]["g"].as_f64().unwrap(), 0.0);
    assert_approx(props["albedo_color"]["b"].as_f64().unwrap(), 0.0);
    // metallic = 0.8
    assert_approx(props["metallic"].as_f64().unwrap(), 0.8);
    // roughness = 0.2
    assert_approx(props["roughness"].as_f64().unwrap(), 0.2);
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_nonexistent_returns_error() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "resource_read",
            json!({"resource_path": "nonexistent/nope.tres"}),
        )
        .unwrap()
        .unwrap_err();

    assert!(
        err.contains("not found") || err.contains("does not exist") || err.contains("Failed")
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_tscn_includes_hint() {
    let f = DirectorFixture::new();
    // test_scene_2d.tscn exists in the test project
    let data = f
        .run(
            "resource_read",
            json!({"resource_path": "test_scene_2d.tscn"}),
        )
        .unwrap()
        .unwrap_data();

    // Should succeed but include a hint
    assert!(data["type"].is_string());
    assert!(data["hint"].as_str().unwrap().contains("scene_read"));
}
