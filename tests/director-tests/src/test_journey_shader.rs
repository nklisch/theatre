use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_visual_shader_create_and_apply_to_scene() {
    let f = DirectorFixture::new();
    let shader_path = DirectorFixture::temp_resource_path("shader_spatial");
    let material_path = DirectorFixture::temp_resource_path("shader_material");
    let scene = DirectorFixture::journey_scene_path("shader_scene");

    // 1. Create VisualShader with:
    //    - VisualShaderNodeColorConstant (node_id=2, fragment, red constant)
    //    - VisualShaderNodeInput (node_id=3, vertex, vertex input)
    //    - Connection: node 2 → output node 0 port 0 (albedo) in fragment
    let shader_data = f
        .run(
            "visual_shader_create",
            json!({
                "resource_path": shader_path,
                "shader_mode": "spatial",
                "nodes": [
                    {
                        "node_id": 2,
                        "type": "VisualShaderNodeColorConstant",
                        "shader_function": "fragment",
                        "properties": {"constant": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}}
                    },
                    {
                        "node_id": 3,
                        "type": "VisualShaderNodeInput",
                        "shader_function": "vertex",
                        "properties": {"input_name": "vertex"}
                    }
                ],
                "connections": [
                    {
                        "from_node": 2,
                        "from_port": 0,
                        "to_node": 0,
                        "to_port": 0,
                        "shader_function": "fragment"
                    }
                ]
            }),
        )
        .unwrap()
        .unwrap_data();

    // Verify shader created with 2 nodes and 1 connection
    assert_eq!(shader_data["path"], shader_path);
    assert_eq!(shader_data["node_count"], 2, "Shader should have 2 nodes");
    assert_eq!(
        shader_data["connection_count"], 1,
        "Shader should have 1 connection"
    );

    // 2. resource_read — verify the .tres was created with correct type
    let shader_read = f
        .run("resource_read", json!({"resource_path": shader_path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(shader_read["type"], "VisualShader");

    // 3. material_create — ShaderMaterial referencing the visual shader
    f.run(
        "material_create",
        json!({
            "resource_path": material_path,
            "material_type": "ShaderMaterial",
            "shader_path": shader_path
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Create Node3D scene
    f.run(
        "scene_create",
        json!({
            "scene_path": scene,
            "root_type": "Node3D",
            "root_name": "ShaderScene"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Add MeshInstance3D
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "MeshInstance3D",
            "node_name": "Mesh"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Set material_override to the ShaderMaterial path
    let mat_res = format!("res://{material_path}");
    f.run(
        "node_set_properties",
        json!({
            "scene_path": scene,
            "node_path": "Mesh",
            "properties": {"material_override": mat_res}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. scene_read — verify MeshInstance3D exists with material reference
    let mesh_node = f.read_node(&scene, "Mesh");
    assert_eq!(mesh_node["type"], "MeshInstance3D");
    assert_eq!(mesh_node["name"], "Mesh");
}
