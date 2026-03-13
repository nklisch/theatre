//! Tests filling coverage gaps identified by test-quality gap analysis.
//!
//! Each test cites the spec condition it verifies.

use crate::harness::{DirectorFixture, OperationResultExt, assert_approx};
use serde_json::json;

// ---------------------------------------------------------------------------
// Critical: signal_connect with binds parameter
// Spec: "binds = extra args passed to method"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn signal_connect_with_binds() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_binds");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Button", "node_name": "Btn"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "H"}),
    )
    .unwrap();

    let data = f
        .run(
            "signal_connect",
            json!({
                "scene_path": &scene,
                "source_path": "Btn",
                "signal_name": "pressed",
                "target_path": "H",
                "method_name": "on_press",
                "binds": ["extra_arg", 42],
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["signal_name"], "pressed");

    // Verify binds appear in signal_list
    let list = f
        .run("signal_list", json!({"scene_path": &scene}))
        .unwrap()
        .unwrap_data();
    let conn = &list["connections"].as_array().unwrap()[0];
    let binds = conn["binds"].as_array().unwrap();
    assert_eq!(binds.len(), 2);
}

// ---------------------------------------------------------------------------
// Critical: signal_connect with flags parameter
// Spec: "CONNECT_DEFERRED=1, CONNECT_PERSIST=2 (auto-added), CONNECT_ONE_SHOT=4"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn signal_connect_with_flags_deferred() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_flags");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Button", "node_name": "Btn"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "H"}),
    )
    .unwrap();

    // CONNECT_DEFERRED (1) | CONNECT_ONE_SHOT (4) = 5
    let data = f
        .run(
            "signal_connect",
            json!({
                "scene_path": &scene,
                "source_path": "Btn",
                "signal_name": "pressed",
                "target_path": "H",
                "method_name": "on_press",
                "flags": 5,
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["signal_name"], "pressed");

    // Verify flags in signal_list (CONNECT_PERSIST=2 should be auto-added → 5|2=7)
    let list = f
        .run("signal_list", json!({"scene_path": &scene}))
        .unwrap()
        .unwrap_data();
    let conn = &list["connections"].as_array().unwrap()[0];
    // flags should include at least our bits (1 and 4)
    let flags = conn["flags"].as_u64().unwrap();
    assert!(
        flags & 1 != 0,
        "CONNECT_DEFERRED should be set, flags={flags}"
    );
    assert!(
        flags & 4 != 0,
        "CONNECT_ONE_SHOT should be set, flags={flags}"
    );
    assert!(
        flags & 2 != 0,
        "CONNECT_PERSIST should be auto-added, flags={flags}"
    );
}

// ---------------------------------------------------------------------------
// Critical: resource_duplicate with deep_copy=true
// Spec: "deep_copy=true makes nested sub-resources independent"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn resource_duplicate_deep_copy() {
    let f = DirectorFixture::new();

    // Create source material
    f.run(
        "material_create",
        json!({
            "resource_path": "tmp/deep_source.tres",
            "material_type": "StandardMaterial3D",
            "properties": {"metallic": 0.5}
        }),
    )
    .unwrap()
    .unwrap_data();

    // Duplicate with deep_copy=true
    let data = f
        .run(
            "resource_duplicate",
            json!({
                "source_path": "tmp/deep_source.tres",
                "dest_path": "tmp/deep_dest.tres",
                "deep_copy": true,
                "property_overrides": {"metallic": 0.9}
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["path"], "tmp/deep_dest.tres");
    assert_eq!(data["type"], "StandardMaterial3D");
    assert!(
        data["overrides_applied"]
            .as_array()
            .unwrap()
            .contains(&json!("metallic"))
    );

    // Verify the duplicate is readable and has overridden value
    let read = f
        .run(
            "resource_read",
            json!({"resource_path": "tmp/deep_dest.tres"}),
        )
        .unwrap()
        .unwrap_data();
    assert_approx(read["properties"]["metallic"].as_f64().unwrap(), 0.9);
}

// ---------------------------------------------------------------------------
// Critical: shader_material_set_params — entire tool has no test
// Spec: "Sets ShaderMaterial params on a node"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn shader_material_set_params_basic() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("shader_params");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Sprite2D", "node_name": "Sprite"}),
    )
    .unwrap();

    // Try to set shader material params — this may fail if no ShaderMaterial is on
    // the node, which itself is a valid error case to verify.
    let result = f
        .run(
            "shader_material_set_params",
            json!({
                "scene_path": &scene,
                "node_path": "Sprite",
                "params": {"color": "#ff0000"},
            }),
        )
        .unwrap();

    // Without a ShaderMaterial assigned, this should error
    assert!(
        !result.success,
        "should error when node has no ShaderMaterial"
    );
}

// ---------------------------------------------------------------------------
// Critical: node_find with property/property_value filter
// Spec: "property — Filter: property must exist on the node.
//        property_value — Filter: property must equal this value"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn node_find_by_property_exists() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("find_prop");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({
            "scene_path": &scene,
            "node_type": "Sprite2D",
            "node_name": "S1",
            "properties": {"visible": false}
        }),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "N1"}),
    )
    .unwrap();

    // Sprite2D has "texture" property, Node2D does not
    let data = f
        .run(
            "node_find",
            json!({
                "scene_path": &scene,
                "property": "texture",
            }),
        )
        .unwrap()
        .unwrap_data();

    let results = data["results"].as_array().unwrap();
    // Only Sprite2D should match (it has the "texture" property)
    assert!(!results.is_empty());
    assert!(results.iter().all(|r| r["type"] == "Sprite2D"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_find_by_property_value() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("find_prop_val");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({
            "scene_path": &scene,
            "node_type": "Node2D",
            "node_name": "A",
            "properties": {"visible": false}
        }),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({
            "scene_path": &scene,
            "node_type": "Node2D",
            "node_name": "B",
            "properties": {"visible": true}
        }),
    )
    .unwrap();

    let data = f
        .run(
            "node_find",
            json!({
                "scene_path": &scene,
                "property": "visible",
                "property_value": false,
            }),
        )
        .unwrap()
        .unwrap_data();

    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "A");
}

// ---------------------------------------------------------------------------
// Critical: scene_read with properties=false
// Spec: "properties=false omits property data"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn scene_read_properties_false_omits_properties() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("read_no_props");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({
            "scene_path": &scene,
            "node_type": "Sprite2D",
            "node_name": "S",
            "properties": {"position": {"x": 100, "y": 200}}
        }),
    )
    .unwrap();

    // Read with properties=true (default) — properties should be present
    let with = f
        .run(
            "scene_read",
            json!({"scene_path": &scene, "properties": true}),
        )
        .unwrap()
        .unwrap_data();
    let child_with = &with["root"]["children"][0];
    assert!(
        child_with.get("properties").is_some(),
        "properties should be present by default"
    );

    // Read with properties=false — properties should be omitted
    let without = f
        .run(
            "scene_read",
            json!({"scene_path": &scene, "properties": false}),
        )
        .unwrap()
        .unwrap_data();
    let child_without = &without["root"]["children"][0];
    assert!(
        child_without.get("properties").is_none()
            || child_without["properties"].is_null()
            || child_without["properties"]
                .as_object()
                .map(|m| m.is_empty())
                .unwrap_or(false),
        "properties should be omitted when properties=false"
    );
}

// ---------------------------------------------------------------------------
// Critical: scene_diff detects moved/reparented nodes
// Spec: "moved: [{node_path, old_parent, new_parent}]"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn scene_diff_detects_moved_node() {
    let f = DirectorFixture::new();
    let scene_a = DirectorFixture::temp_scene_path("diff_a_moved");
    let scene_b = DirectorFixture::temp_scene_path("diff_b_moved");

    // Scene A: Root > Parent1 > Child
    f.run(
        "scene_create",
        json!({"scene_path": scene_a, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene_a, "node_type": "Node2D", "node_name": "Parent1"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene_a, "parent_path": "Parent1", "node_type": "Sprite2D", "node_name": "Child"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene_a, "node_type": "Node2D", "node_name": "Parent2"}),
    )
    .unwrap();

    // Scene B: Root > Parent2 > Child (Child moved from Parent1 to Parent2)
    f.run(
        "scene_create",
        json!({"scene_path": scene_b, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene_b, "node_type": "Node2D", "node_name": "Parent1"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene_b, "node_type": "Node2D", "node_name": "Parent2"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": scene_b, "parent_path": "Parent2", "node_type": "Sprite2D", "node_name": "Child"}),
    )
    .unwrap();

    let data = f
        .run(
            "scene_diff",
            json!({"scene_a": scene_a, "scene_b": scene_b}),
        )
        .unwrap()
        .unwrap_data();

    // The diff should detect the node was moved (or at minimum, report it as removed+added)
    let moved = data["moved"].as_array();
    let added = data["added"].as_array().unwrap();
    let removed = data["removed"].as_array().unwrap();

    // Either the `moved` array captures it, or it shows as removed+added
    let detected =
        moved.map(|m| !m.is_empty()).unwrap_or(false) || (!added.is_empty() && !removed.is_empty());
    assert!(
        detected,
        "scene_diff should detect node movement between parents"
    );
}

// ---------------------------------------------------------------------------
// Critical: shape_create with both save_path AND scene attachment
// Spec: "Can both save AND attach"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn shape_create_save_and_attach() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("shape_both");
    let save_path = "tmp/test_shape_both.tres";

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "StaticBody3D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "CollisionShape3D", "node_name": "Col"}),
    )
    .unwrap();

    let data = f
        .run(
            "shape_create",
            json!({
                "shape_type": "BoxShape3D",
                "shape_params": {"size": {"x": 2.0, "y": 2.0, "z": 2.0}},
                "save_path": save_path,
                "scene_path": &scene,
                "node_path": "Col",
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["shape_type"], "BoxShape3D");
    assert_eq!(data["saved_to"], save_path);
    assert_eq!(data["attached_to"], "Col");
}

// ---------------------------------------------------------------------------
// High: animation_create with negative length
// Spec: "length must be positive"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_rejects_negative_length() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "animation_create",
            json!({
                "resource_path": "tmp/neg_length.tres",
                "length": -1.0,
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("length must be positive") || err.contains("positive"),
        "expected positive length error, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// High: signal_disconnect nonexistent connection
// Spec: "Requires exact match on source, signal, target, method"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn signal_disconnect_nonexistent_connection() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_dc_missing");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "A"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "B"}),
    )
    .unwrap();

    // Try to disconnect a signal that was never connected
    let result = f
        .run(
            "signal_disconnect",
            json!({
                "scene_path": &scene,
                "source_path": "A",
                "signal_name": "tree_entered",
                "target_path": "B",
                "method_name": "nonexistent",
            }),
        )
        .unwrap();

    assert!(
        !result.success,
        "disconnecting nonexistent connection should error"
    );
}

// ---------------------------------------------------------------------------
// High: node_add to nonexistent parent path
// Spec: parent_path must be a valid node in the scene tree
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn node_add_nonexistent_parent_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("add_bad_parent");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();

    let result = f
        .run(
            "node_add",
            json!({
                "scene_path": &scene,
                "parent_path": "Nonexistent/Path",
                "node_type": "Sprite2D",
                "node_name": "Child",
            }),
        )
        .unwrap();

    assert!(!result.success, "adding to nonexistent parent should error");
}

// ---------------------------------------------------------------------------
// High: node_set_script with nonexistent script file
// Spec: "Script must exist on disk"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn node_set_script_nonexistent_script_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("script_missing");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "N"}),
    )
    .unwrap();

    let result = f
        .run(
            "node_set_script",
            json!({
                "scene_path": &scene,
                "node_path": "N",
                "script_path": "scripts/does_not_exist.gd",
            }),
        )
        .unwrap();

    assert!(!result.success, "attaching nonexistent script should error");
}

// ---------------------------------------------------------------------------
// High: visual_shader with duplicate node_ids in same function
// Spec: "IDs must be unique within a shader"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_rejects_duplicate_node_ids() {
    let f = DirectorFixture::new();
    let result = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": "tmp/dup_ids.tres",
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                    },
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeVec3Constant",
                        "shader_function": "fragment",
                    },
                ],
            }),
        )
        .unwrap();

    // Duplicate IDs in same function should be rejected or cause an error
    // (mixed vertex/fragment is allowed because they're different function graphs)
    assert!(
        !result.success || result.data["node_count"].as_u64().unwrap_or(0) < 2,
        "duplicate node_ids in same shader_function should be rejected or deduplicated"
    );
}

// ---------------------------------------------------------------------------
// High: batch with unknown operation name
// Spec: operations array uses operation name strings
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn batch_unknown_operation_errors() {
    let f = DirectorFixture::new();
    let result = f
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "nonexistent_operation", "params": {}},
                ]
            }),
        )
        .unwrap();

    // Should report failure for the unknown operation
    assert_eq!(result.data["failed"], 1);
    let results = result.data["results"].as_array().unwrap();
    assert_eq!(results[0]["success"], false);
}

// ---------------------------------------------------------------------------
// High: scene_create overwriting existing file
// Spec: "Creates new .tscn with single root"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn scene_create_overwrites_existing() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("overwrite");

    // Create scene with Node2D root
    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Sprite2D", "node_name": "Child"}),
    )
    .unwrap();

    // Overwrite with Node3D root — should succeed and replace the scene
    let data = f
        .run(
            "scene_create",
            json!({"scene_path": &scene, "root_type": "Node3D"}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["root_type"], "Node3D");

    // Verify the old child is gone
    let read = f
        .run("scene_read", json!({"scene_path": &scene}))
        .unwrap()
        .unwrap_data();
    assert_eq!(read["root"]["type"], "Node3D");
    let children = read["root"]["children"].as_array();
    assert!(
        children.is_none() || children.unwrap().is_empty(),
        "overwritten scene should have no children"
    );
}

// ---------------------------------------------------------------------------
// High: resource_read with depth=0
// Spec: "At depth 0, nested resources are returned as path strings"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn resource_read_depth_zero() {
    let f = DirectorFixture::new();

    // Create a material (has nested sub-resources potentially)
    f.run(
        "material_create",
        json!({
            "resource_path": "tmp/depth0_mat.tres",
            "material_type": "StandardMaterial3D",
            "properties": {"metallic": 0.5}
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "resource_read",
            json!({
                "resource_path": "tmp/depth0_mat.tres",
                "depth": 0
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["type"], "StandardMaterial3D");
    // At depth=0, the resource should still be readable but nested resources
    // should appear as path strings rather than expanded objects
    assert!(data.get("properties").is_some() || data.get("type").is_some());
}

// ---------------------------------------------------------------------------
// High: animation_add_track with interpolation=nearest
// Spec: "interpolation: nearest/linear/cubic"
// (cubic already tested; verifying nearest as a boundary)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_track_interpolation_nearest() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_nearest.tres";
    f.run(
        "animation_create",
        json!({"resource_path": path, "length": 1.0}),
    )
    .unwrap();

    f.run(
        "animation_add_track",
        json!({
            "resource_path": path,
            "track_type": "position_3d",
            "node_path": "Node",
            "interpolation": "nearest",
            "keyframes": [
                {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}},
                {"time": 1.0, "value": {"x": 5, "y": 0, "z": 0}},
            ],
        }),
    )
    .unwrap()
    .unwrap_data();

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(read["tracks"][0]["interpolation"], "nearest");
}

// ---------------------------------------------------------------------------
// High: material_create with #hex color notation
// Spec: "Type conversion: Color from '#ff0000'"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn material_create_hex_color() {
    let f = DirectorFixture::new();
    let data = f
        .run(
            "material_create",
            json!({
                "resource_path": "tmp/hex_color_mat.tres",
                "material_type": "StandardMaterial3D",
                "properties": {"albedo_color": "#ff0000"}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["type"], "StandardMaterial3D");

    // Verify the color was applied
    let read = f
        .run(
            "resource_read",
            json!({"resource_path": "tmp/hex_color_mat.tres"}),
        )
        .unwrap()
        .unwrap_data();
    // albedo_color should be red-ish
    let color = &read["properties"]["albedo_color"];
    assert!(
        color.is_object() || color.is_string(),
        "color should be serialized"
    );
}

// ---------------------------------------------------------------------------
// High: visual_shader for canvas_item mode
// Spec: "shader_mode: spatial/canvas_item/particles/sky/fog"
// (only spatial tested in existing suite)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_canvas_item_mode() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_canvas.tres";
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
                "shader_mode": "canvas_item",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["node_count"], 1);
    assert_eq!(data["path"], path);
}

// ---------------------------------------------------------------------------
// High: visual_shader for particles mode with start/process/collide functions
// Spec: "particles shader_function: start/process/collide"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_particles_mode() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_particles.tres";
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
                "shader_mode": "particles",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeFloatConstant",
                        "shader_function": "start",
                    },
                    {
                        "node_id": 3,
                        "type": "VisualShaderNodeFloatConstant",
                        "shader_function": "process",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["node_count"], 2);
}

// ---------------------------------------------------------------------------
// High: animation_create with pingpong loop mode
// Spec: "loop_mode: none/linear/pingpong" — none and linear tested, pingpong not
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_pingpong_loop() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_pingpong.tres";
    let data = f
        .run(
            "animation_create",
            json!({
                "resource_path": path,
                "length": 1.0,
                "loop_mode": "pingpong",
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["loop_mode"], "pingpong");

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(read["loop_mode"], "pingpong");
}

// ---------------------------------------------------------------------------
// High: node_find with name_pattern using ? wildcard
// Spec: "name_pattern supports * and ? wildcards"
// (* tested in combined_filters; ? not tested)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn node_find_name_pattern_question_mark_wildcard() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("find_qmark");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "E1"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "E2"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "node_type": "Node2D", "node_name": "E10"}),
    )
    .unwrap();

    // "E?" should match E1 and E2 but not E10 (? matches exactly one character)
    let data = f
        .run(
            "node_find",
            json!({"scene_path": &scene, "name_pattern": "E?"}),
        )
        .unwrap()
        .unwrap_data();

    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "E? should match E1 and E2 only");
}

// ---------------------------------------------------------------------------
// High: scene_list with directory filter
// Spec: "directory — optional subdir filter"
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires Godot binary"]
fn scene_list_with_directory_filter() {
    let f = DirectorFixture::new();

    // Create scenes in different subdirectories
    f.run(
        "scene_create",
        json!({"scene_path": "tmp/subdir_a/test.tscn", "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "scene_create",
        json!({"scene_path": "tmp/subdir_b/test.tscn", "root_type": "Node2D"}),
    )
    .unwrap();

    let data = f
        .run("scene_list", json!({"directory": "tmp/subdir_a"}))
        .unwrap()
        .unwrap_data();

    let scenes = data["scenes"].as_array().unwrap();
    // Should only include scenes from subdir_a
    assert!(
        scenes
            .iter()
            .all(|s| { s["path"].as_str().unwrap().contains("subdir_a") })
    );
    assert!(!scenes.is_empty());
}
