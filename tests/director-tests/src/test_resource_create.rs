use crate::harness::{DirectorFixture, assert_approx, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn material_create_standard_material_3d() {
    let f = DirectorFixture::new();
    let path = "tmp/test_mat_standard.tres";
    let data = f
        .run(
            "material_create",
            json!({
                "resource_path": path,
                "material_type": "StandardMaterial3D",
                "properties": {
                    "albedo_color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0},
                    "metallic": 0.8
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["path"], path);
    assert_eq!(data["type"], "StandardMaterial3D");

    // Verify via resource_read
    let read = f
        .run("resource_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(read["type"], "StandardMaterial3D");
    assert_approx(read["properties"]["metallic"].as_f64().unwrap(), 0.8);
}

#[test]
#[ignore = "requires Godot binary"]
fn material_create_rejects_non_material() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "material_create",
            json!({
                "resource_path": "tmp/bad.tres",
                "material_type": "Node2D"
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not a Material subclass"));
}

#[test]
#[ignore = "requires Godot binary"]
fn material_create_shader_material_with_shader_path() {
    // This test requires a .gdshader file to exist in the test project.
    let f = DirectorFixture::new();
    let result = f
        .run(
            "material_create",
            json!({
                "resource_path": "tmp/test_shader_mat.tres",
                "material_type": "ShaderMaterial",
                "shader_path": "test_shader.gdshader"
            }),
        )
        .unwrap();
    // If the shader file doesn't exist, this will error — that's fine,
    // it validates the path-checking logic.
    // If it does exist, verify success.
    if result.success {
        assert_eq!(result.data["type"], "ShaderMaterial");
    } else {
        assert!(result.error.unwrap().contains("Shader not found"));
    }
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_save_to_file() {
    let f = DirectorFixture::new();
    let path = "tmp/test_box_shape.tres";
    let data = f
        .run(
            "shape_create",
            json!({
                "shape_type": "BoxShape3D",
                "shape_params": {"size": {"x": 2.0, "y": 3.0, "z": 4.0}},
                "save_path": path
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["shape_type"], "BoxShape3D");
    assert_eq!(data["saved_to"], path);

    // Verify via resource_read
    let read = f
        .run("resource_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(read["type"], "BoxShape3D");
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_attach_to_collision_node() {
    let f = DirectorFixture::new();
    // Create a scene with a CollisionShape3D
    let scene = "tmp/test_shape_attach.tscn";
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "StaticBody3D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "CollisionShape3D",
            "node_name": "Collision"
        }),
    )
    .unwrap()
    .unwrap_data();

    // Attach a shape
    let data = f
        .run(
            "shape_create",
            json!({
                "shape_type": "SphereShape3D",
                "shape_params": {"radius": 2.5},
                "scene_path": scene,
                "node_path": "Collision"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["shape_type"], "SphereShape3D");
    assert_eq!(data["attached_to"], "Collision");

    // Verify via scene_read
    let read = f
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let collision = &read["root"]["children"][0];
    assert_eq!(collision["name"], "Collision");
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_2d() {
    let f = DirectorFixture::new();
    let path = "tmp/test_circle_shape.tres";
    let data = f
        .run(
            "shape_create",
            json!({
                "shape_type": "CircleShape2D",
                "shape_params": {"radius": 50.0},
                "save_path": path
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["shape_type"], "CircleShape2D");
    assert_eq!(data["saved_to"], path);
}

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_rejects_no_output() {
    let f = DirectorFixture::new();
    let err = f
        .run("shape_create", json!({"shape_type": "BoxShape3D"}))
        .unwrap()
        .unwrap_err();
    assert!(err.contains("At least one of save_path or scene_path"));
}

#[test]
#[ignore = "requires Godot binary"]
fn style_box_create_flat() {
    let f = DirectorFixture::new();
    let path = "tmp/test_stylebox.tres";
    let data = f
        .run(
            "style_box_create",
            json!({
                "resource_path": path,
                "style_type": "StyleBoxFlat",
                "properties": {
                    "bg_color": "#336699",
                    "corner_radius_top_left": 8,
                    "corner_radius_top_right": 8
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["path"], path);
    assert_eq!(data["type"], "StyleBoxFlat");
}

#[test]
#[ignore = "requires Godot binary"]
fn style_box_create_rejects_invalid_type() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "style_box_create",
            json!({
                "resource_path": "tmp/bad.tres",
                "style_type": "StyleBoxFancy"
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("Invalid style_type"));
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_duplicate_shallow() {
    let f = DirectorFixture::new();
    // Create source material
    f.run(
        "material_create",
        json!({
            "resource_path": "tmp/dup_source.tres",
            "material_type": "StandardMaterial3D",
            "properties": {"metallic": 0.5}
        }),
    )
    .unwrap()
    .unwrap_data();

    // Duplicate with override
    let data = f
        .run(
            "resource_duplicate",
            json!({
                "source_path": "tmp/dup_source.tres",
                "dest_path": "tmp/dup_dest.tres",
                "property_overrides": {"metallic": 0.9}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["path"], "tmp/dup_dest.tres");
    assert_eq!(data["type"], "StandardMaterial3D");
    assert!(
        data["overrides_applied"]
            .as_array()
            .unwrap()
            .contains(&json!("metallic"))
    );

    // Verify override took effect
    let read = f
        .run(
            "resource_read",
            json!({"resource_path": "tmp/dup_dest.tres"}),
        )
        .unwrap()
        .unwrap_data();
    assert_approx(read["properties"]["metallic"].as_f64().unwrap(), 0.9);
}

#[test]
#[ignore = "requires Godot binary"]
fn resource_duplicate_not_found() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "resource_duplicate",
            json!({
                "source_path": "nonexistent.tres",
                "dest_path": "tmp/dup_out.tres"
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not found"));
}
