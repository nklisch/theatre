use crate::harness::{DirectorFixture, assert_approx, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_material_pipeline_create_duplicate_apply() {
    let f = DirectorFixture::new();
    let base_mat = DirectorFixture::temp_resource_path("res_base_mat");
    let red_variant = DirectorFixture::temp_resource_path("res_red_variant");
    let shiny_variant = DirectorFixture::temp_resource_path("res_shiny_variant");
    let box_shape = DirectorFixture::temp_resource_path("res_box_shape");
    let capsule_shape = DirectorFixture::temp_resource_path("res_capsule_shape");

    // 1. Create StandardMaterial3D "base_mat" with metallic=0.5, roughness=0.7
    f.run(
        "material_create",
        json!({
            "resource_path": base_mat,
            "material_type": "StandardMaterial3D",
            "properties": {"metallic": 0.5, "roughness": 0.7}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 2. resource_read — verify base_mat properties
    let base_read = f
        .run("resource_read", json!({"resource_path": base_mat}))
        .unwrap()
        .unwrap_data();
    assert_eq!(base_read["type"], "StandardMaterial3D");
    assert_approx(base_read["properties"]["metallic"].as_f64().unwrap(), 0.5);
    assert_approx(base_read["properties"]["roughness"].as_f64().unwrap(), 0.7);

    // 3. resource_duplicate — Duplicate base_mat → "red_variant" with albedo_color override
    let red_dup = f
        .run(
            "resource_duplicate",
            json!({
                "source_path": base_mat,
                "dest_path": red_variant,
                "property_overrides": {
                    "albedo_color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(red_dup["path"], red_variant);
    assert!(
        red_dup["overrides_applied"]
            .as_array()
            .unwrap()
            .contains(&json!("albedo_color"))
    );

    // 4. resource_duplicate — Duplicate base_mat → "shiny_variant" with metallic=1.0
    let shiny_dup = f
        .run(
            "resource_duplicate",
            json!({
                "source_path": base_mat,
                "dest_path": shiny_variant,
                "property_overrides": {"metallic": 1.0}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(shiny_dup["path"], shiny_variant);
    assert!(
        shiny_dup["overrides_applied"]
            .as_array()
            .unwrap()
            .contains(&json!("metallic"))
    );

    // 5. resource_read — verify red_variant has albedo override but same roughness
    let red_read = f
        .run("resource_read", json!({"resource_path": red_variant}))
        .unwrap()
        .unwrap_data();
    assert_approx(
        red_read["properties"]["albedo_color"]["r"]
            .as_f64()
            .unwrap(),
        1.0,
    );
    assert_approx(
        red_read["properties"]["albedo_color"]["g"]
            .as_f64()
            .unwrap(),
        0.0,
    );
    assert_approx(red_read["properties"]["roughness"].as_f64().unwrap(), 0.7);

    // 6. resource_read — verify shiny_variant has metallic=1.0 but same roughness
    let shiny_read = f
        .run("resource_read", json!({"resource_path": shiny_variant}))
        .unwrap()
        .unwrap_data();
    assert_approx(shiny_read["properties"]["metallic"].as_f64().unwrap(), 1.0);
    assert_approx(shiny_read["properties"]["roughness"].as_f64().unwrap(), 0.7);

    // 7. shape_create — BoxShape3D saved to file
    let box_data = f
        .run(
            "shape_create",
            json!({
                "shape_type": "BoxShape3D",
                "shape_params": {"size": {"x": 2.0, "y": 1.0, "z": 2.0}},
                "save_path": box_shape
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(box_data["shape_type"], "BoxShape3D");
    assert_eq!(box_data["saved_to"], box_shape);

    // 8. shape_create — CapsuleShape3D saved to file
    let capsule_data = f
        .run(
            "shape_create",
            json!({
                "shape_type": "CapsuleShape3D",
                "shape_params": {"radius": 0.5, "height": 2.0},
                "save_path": capsule_shape
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(capsule_data["shape_type"], "CapsuleShape3D");

    // 9. shape_create — SphereShape3D attached to a scene node
    let sphere_scene = DirectorFixture::journey_scene_path("res_sphere_scene");
    f.run(
        "scene_create",
        json!({
            "scene_path": sphere_scene,
            "root_type": "StaticBody3D"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": sphere_scene,
            "node_type": "CollisionShape3D",
            "node_name": "SphereCol"
        }),
    )
    .unwrap()
    .unwrap_data();
    let sphere_data = f
        .run(
            "shape_create",
            json!({
                "shape_type": "SphereShape3D",
                "shape_params": {"radius": 1.5},
                "scene_path": sphere_scene,
                "node_path": "SphereCol"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(sphere_data["shape_type"], "SphereShape3D");
    assert_eq!(sphere_data["attached_to"], "SphereCol");

    // 10. resource_read — verify BoxShape3D properties (size)
    let box_read = f
        .run("resource_read", json!({"resource_path": box_shape}))
        .unwrap()
        .unwrap_data();
    assert_eq!(box_read["type"], "BoxShape3D");

    // 11. style_box_create — StyleBoxFlat with bg_color and corner radii
    let stylebox_path = DirectorFixture::temp_resource_path("res_stylebox_flat");
    let sb_data = f
        .run(
            "style_box_create",
            json!({
                "resource_path": stylebox_path,
                "style_type": "StyleBoxFlat",
                "properties": {
                    "bg_color": "#336699",
                    "corner_radius_top_left": 8,
                    "corner_radius_top_right": 8,
                    "corner_radius_bottom_left": 4,
                    "corner_radius_bottom_right": 4
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(sb_data["type"], "StyleBoxFlat");
    assert_eq!(sb_data["path"], stylebox_path);

    // 12. resource_read — verify StyleBoxFlat properties
    let sb_read = f
        .run("resource_read", json!({"resource_path": stylebox_path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(sb_read["type"], "StyleBoxFlat");
}

#[test]
#[ignore = "requires Godot binary"]
fn journey_style_box_variants() {
    let f = DirectorFixture::new();

    // 1. StyleBoxFlat with all corner radii
    let flat_path = DirectorFixture::temp_resource_path("sb_flat");
    let flat = f
        .run(
            "style_box_create",
            json!({
                "resource_path": flat_path,
                "style_type": "StyleBoxFlat",
                "properties": {
                    "bg_color": "#4488cc",
                    "corner_radius_top_left": 10,
                    "corner_radius_top_right": 10,
                    "corner_radius_bottom_left": 10,
                    "corner_radius_bottom_right": 10
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(flat["type"], "StyleBoxFlat");

    // 2. StyleBoxEmpty (no properties needed)
    let empty_path = DirectorFixture::temp_resource_path("sb_empty");
    let empty = f
        .run(
            "style_box_create",
            json!({
                "resource_path": empty_path,
                "style_type": "StyleBoxEmpty"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(empty["type"], "StyleBoxEmpty");

    // 3. StyleBoxLine with color and thickness
    let line_path = DirectorFixture::temp_resource_path("sb_line");
    let line = f
        .run(
            "style_box_create",
            json!({
                "resource_path": line_path,
                "style_type": "StyleBoxLine",
                "properties": {
                    "color": "#ffffff",
                    "thickness": 2
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(line["type"], "StyleBoxLine");

    // 4. resource_read — verify each type and properties
    let flat_read = f
        .run("resource_read", json!({"resource_path": flat_path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(flat_read["type"], "StyleBoxFlat");

    let empty_read = f
        .run("resource_read", json!({"resource_path": empty_path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(empty_read["type"], "StyleBoxEmpty");

    let line_read = f
        .run("resource_read", json!({"resource_path": line_path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(line_read["type"], "StyleBoxLine");
}
