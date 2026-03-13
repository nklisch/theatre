use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_physics_layer_setup() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("physics_layers");

    // 1. physics_set_layer_names — Name 2d_physics layers
    let layer_names = f
        .run(
            "physics_set_layer_names",
            json!({
                "layer_type": "2d_physics",
                "layers": {
                    "1": "player",
                    "2": "enemies",
                    "3": "environment",
                    "4": "projectiles"
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(layer_names["layer_type"], "2d_physics");
    assert_eq!(layer_names["layers_set"], 4);

    // 2. Create Game scene
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D", "root_name": "Game"}),
    )
    .unwrap()
    .unwrap_data();

    // 3. Add CharacterBody2D "Player"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "CharacterBody2D",
            "node_name": "Player"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Add CharacterBody2D "Enemy"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "CharacterBody2D",
            "node_name": "Enemy"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Add StaticBody2D "Wall"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "StaticBody2D",
            "node_name": "Wall"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Add Area2D "Bullet"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Area2D",
            "node_name": "Bullet"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. physics_set_layers — Player: layer=1, mask=2|3 (collides with enemies + environment)
    // layer 1 = bit 0 = value 1
    // mask 2|3 = bits 1+2 = value 6
    let player_phys = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene,
                "node_path": "Player",
                "collision_layer": 1,
                "collision_mask": 6
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(player_phys["collision_layer"], 1);
    assert_eq!(player_phys["collision_mask"], 6);

    // 8. physics_set_layers — Enemy: layer=2, mask=1|3|4 (layers 1+3+4)
    // layer 2 = value 2; mask layers 1+3+4 = bits 0+2+3 = values 1+4+8 = 13
    let enemy_phys = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene,
                "node_path": "Enemy",
                "collision_layer": 2,
                "collision_mask": 13
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(enemy_phys["collision_layer"], 2);
    assert_eq!(enemy_phys["collision_mask"], 13);

    // 9. physics_set_layers — Wall: layer=4 (layer 3 = bit 2 = value 4), mask=0
    let wall_phys = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene,
                "node_path": "Wall",
                "collision_layer": 4,
                "collision_mask": 0
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(wall_phys["collision_layer"], 4);

    // 10. physics_set_layers — Bullet: layer=8 (layer 4 = bit 3 = value 8), mask=2 (enemies only)
    let bullet_phys = f
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene,
                "node_path": "Bullet",
                "collision_layer": 8,
                "collision_mask": 2
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(bullet_phys["collision_layer"], 8);
    assert_eq!(bullet_phys["collision_mask"], 2);

    // 11. scene_read — verify all nodes exist
    let player = f.read_node(&scene, "Player");
    assert_eq!(player["type"], "CharacterBody2D");

    let enemy = f.read_node(&scene, "Enemy");
    assert_eq!(enemy["type"], "CharacterBody2D");

    let wall = f.read_node(&scene, "Wall");
    assert_eq!(wall["type"], "StaticBody2D");

    let bullet = f.read_node(&scene, "Bullet");
    assert_eq!(bullet["type"], "Area2D");
}
