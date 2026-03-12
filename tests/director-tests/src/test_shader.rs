use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_empty() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_empty.tres";
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
                "shader_mode": "spatial",
                "nodes": [],
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["path"], path);
    assert_eq!(data["node_count"], 0);
    assert_eq!(data["connection_count"], 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_with_nodes() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_nodes.tres";
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                    },
                    {
                        "node_id": 3,
                        "type": "VisualShaderNodeVec3Constant",
                        "shader_function": "vertex",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["node_count"], 2);
    assert_eq!(data["connection_count"], 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_with_connections() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_conn.tres";
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                    },
                ],
                "connections": [
                    {
                        "from_node": 2,
                        "from_port": 0,
                        "to_node": 0,
                        "to_port": 0,
                        "shader_function": "fragment",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["node_count"], 1);
    assert_eq!(data["connection_count"], 1);
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_fragment_nodes() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_fragment.tres";
    // Spatial shader with a color constant connected to albedo (port 0)
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                        "position": [100.0, 100.0],
                    },
                ],
                "connections": [
                    {
                        "from_node": 2,
                        "from_port": 0,
                        "to_node": 0,
                        "to_port": 0,
                        "shader_function": "fragment",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["node_count"], 1);
    assert_eq!(data["connection_count"], 1);
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_mixed_vertex_fragment() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_mixed.tres";
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
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
                        "shader_function": "vertex",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_data();
    // Two nodes total across both function graphs
    assert_eq!(data["node_count"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_invalid_shader_function() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": "tmp/bad_func.tres",
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "invalid",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("Invalid shader_function"),
        "expected shader_function error, got: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_invalid_mode() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": "tmp/bad_mode.tres",
                "shader_mode": "invalid",
                "nodes": [],
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("Invalid shader_mode"),
        "expected shader_mode error, got: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_invalid_node_type() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": "tmp/bad_type.tres",
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "NotAClass",
                        "shader_function": "fragment",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("Unknown class") || err.contains("NotAClass"),
        "expected unknown class error, got: {err}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_rejects_reserved_node_id() {
    let f = DirectorFixture::new();
    // node_id = 0 is reserved
    let err = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": "tmp/reserved_id.tres",
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 0,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("reserved") || err.contains(">= 2"),
        "expected reserved id error, got: {err}"
    );

    // node_id = 1 is also reserved
    let err2 = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": "tmp/reserved_id1.tres",
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 1,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(
        err2.contains("reserved") || err2.contains(">= 2"),
        "expected reserved id error, got: {err2}"
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn visual_shader_create_sets_node_properties() {
    let f = DirectorFixture::new();
    let path = "tmp/test_shader_props.tres";
    // Create VisualShaderNodeInput with input_name = "vertex"
    let data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": path,
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeInput",
                        "shader_function": "vertex",
                        "properties": {
                            "input_name": "vertex",
                        },
                    },
                ],
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["node_count"], 1);
    assert_eq!(data["path"], path);
}
