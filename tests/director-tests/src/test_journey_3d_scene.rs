use crate::harness::{DirectorFixture, assert_approx};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_build_3d_scene_with_materials_and_physics() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("3d_scene");
    let floor_mat = DirectorFixture::temp_resource_path("3d_floor_mat");
    let player_mat = DirectorFixture::temp_resource_path("3d_player_mat");

    // 1. Create Node3D root "World"
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node3D", "root_name": "World"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. Create StandardMaterial3D "floor_mat" with albedo gray, roughness 0.9
    f.run(
        "material_create",
        json!({
            "resource_path": floor_mat,
            "material_type": "StandardMaterial3D",
            "properties": {
                "albedo_color": {"r": 0.5, "g": 0.5, "b": 0.5, "a": 1.0},
                "roughness": 0.9
            }
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Create StandardMaterial3D "player_mat" with albedo red, metallic 0.8
    f.run(
        "material_create",
        json!({
            "resource_path": player_mat,
            "material_type": "StandardMaterial3D",
            "properties": {
                "albedo_color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0},
                "metallic": 0.8
            }
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Add StaticBody3D "Floor"
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "StaticBody3D", "node_name": "Floor"}),
    )
    .unwrap()
    .unwrap_data();

    // 5. Add CollisionShape3D "FloorCol" under Floor
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "Floor",
            "node_type": "CollisionShape3D",
            "node_name": "FloorCol"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Create BoxShape3D (20x0.2x20) attached to Floor/FloorCol
    f.run(
        "shape_create",
        json!({
            "shape_type": "BoxShape3D",
            "shape_params": {"size": {"x": 20.0, "y": 0.2, "z": 20.0}},
            "scene_path": scene,
            "node_path": "Floor/FloorCol"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. Add MeshInstance3D "FloorMesh" under Floor
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "Floor",
            "node_type": "MeshInstance3D",
            "node_name": "FloorMesh"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 8. Set FloorMesh material_override to floor_mat path (using res:// prefix)
    let floor_mat_res = format!("res://{floor_mat}");
    f.run(
        "node_set_properties",
        json!({
            "scene_path": scene,
            "node_path": "Floor/FloorMesh",
            "properties": {"material_override": floor_mat_res}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 9. Add CharacterBody3D "Player"
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "CharacterBody3D", "node_name": "Player"}),
    )
    .unwrap()
    .unwrap_data();

    // 10. Add CollisionShape3D "PlayerCol" under Player
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "Player",
            "node_type": "CollisionShape3D",
            "node_name": "PlayerCol"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 11. Create CapsuleShape3D (radius=0.4, height=1.8) attached to Player/PlayerCol
    f.run(
        "shape_create",
        json!({
            "shape_type": "CapsuleShape3D",
            "shape_params": {"radius": 0.4, "height": 1.8},
            "scene_path": scene,
            "node_path": "Player/PlayerCol"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 12. Set Player position to (0, 1, 0)
    f.run(
        "node_set_properties",
        json!({
            "scene_path": scene,
            "node_path": "Player",
            "properties": {"position": {"x": 0, "y": 1, "z": 0}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 13. physics_set_layers — Player collision_layer=1, collision_mask=3
    let player_phys = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene,
                "node_path": "Player",
                "collision_layer": 1,
                "collision_mask": 3
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(player_phys["collision_layer"], 1);
    assert_eq!(player_phys["collision_mask"], 3);

    // 14. physics_set_layers — Floor collision_layer=2
    let floor_phys = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene,
                "node_path": "Floor",
                "collision_layer": 2,
                "collision_mask": 0
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(floor_phys["collision_layer"], 2);

    // 15. resource_read — verify player_mat metallic=0.8
    let mat_data = f
        .run("resource_read", json!({"resource_path": player_mat}))
        .unwrap()
        .unwrap_data();
    assert_eq!(mat_data["type"], "StandardMaterial3D");
    assert_approx(mat_data["properties"]["metallic"].as_f64().unwrap(), 0.8);

    // 16. scene_read — verify full hierarchy
    let player_node = f.read_node(&scene, "Player");
    assert_eq!(player_node["type"], "CharacterBody3D");
    assert_approx(
        player_node["properties"]["position"]["y"].as_f64().unwrap(),
        1.0,
    );

    let floor_node = f.read_node(&scene, "Floor");
    assert_eq!(floor_node["type"], "StaticBody3D");

    let floor_col = f.read_node(&scene, "Floor/FloorCol");
    assert_eq!(floor_col["type"], "CollisionShape3D");

    let player_col = f.read_node(&scene, "Player/PlayerCol");
    assert_eq!(player_col["type"], "CollisionShape3D");
}
