use crate::harness::{DirectorFixture, assert_approx, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_build_2d_platformer_level() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("platformer_level");

    // 1. Create scene with Node2D root "Level"
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D", "root_name": "Level"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. Add CharacterBody2D "Player"
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "CharacterBody2D", "node_name": "Player"}),
    )
    .unwrap()
    .unwrap_data();

    // 3. Add CollisionShape2D "Hitbox" under Player
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "Player",
            "node_type": "CollisionShape2D",
            "node_name": "Hitbox"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. shape_create — CapsuleShape2D attached to Player/Hitbox
    f.run(
        "shape_create",
        json!({
            "shape_type": "CapsuleShape2D",
            "shape_params": {"radius": 16.0, "height": 48.0},
            "scene_path": scene,
            "node_path": "Player/Hitbox"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Add Sprite2D "PlayerSprite" under Player
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "Player",
            "node_type": "Sprite2D",
            "node_name": "PlayerSprite"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Set Player position to (64, 192)
    f.run(
        "node_set_properties",
        json!({
            "scene_path": scene,
            "node_path": "Player",
            "properties": {"position": {"x": 64, "y": 192}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. Add TileMapLayer "Ground" with tileset
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "TileMapLayer",
            "node_name": "Ground",
            "properties": {"tile_set": "res://fixtures/test_tileset.tres"}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 8. Place ground tiles across bottom row (y=0, x=0..15)
    let ground_cells: Vec<serde_json::Value> = (0..16)
        .map(|x| json!({"coords": [x, 0], "source_id": 0, "atlas_coords": [0, 0]}))
        .collect();
    let set_data = f
        .run(
            "tilemap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground",
                "cells": ground_cells
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(set_data["cells_set"], 16);

    // 9. Place platform tiles at elevated positions
    let platform_cells = vec![
        json!({"coords": [4, -3], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [5, -3], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [6, -3], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [10, -5], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [11, -5], "source_id": 0, "atlas_coords": [0, 0]}),
    ];
    f.run(
        "tilemap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Ground",
            "cells": platform_cells
        }),
    )
    .unwrap()
    .unwrap_data();

    // 10. Add Node2D "Enemies" container
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Enemies"}),
    )
    .unwrap()
    .unwrap_data();

    // 11. Add CharacterBody2D "Goomba" under Enemies
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "Enemies",
            "node_type": "CharacterBody2D",
            "node_name": "Goomba"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 12. Position Goomba at (320, 192)
    f.run(
        "node_set_properties",
        json!({
            "scene_path": scene,
            "node_path": "Enemies/Goomba",
            "properties": {"position": {"x": 320, "y": 192}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 13. Add Player to "player" group, Goomba to "enemies" group
    f.run(
        "node_set_groups",
        json!({
            "scene_path": scene,
            "node_path": "Player",
            "add": ["player"]
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_set_groups",
        json!({
            "scene_path": scene,
            "node_path": "Enemies/Goomba",
            "add": ["enemies"]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 14. Add Area2D "Coin" with CollisionShape2D under it
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Area2D", "node_name": "Coin"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "Coin",
            "node_type": "CollisionShape2D",
            "node_name": "CoinHitbox"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 15. Add Coin to "collectibles" group
    f.run(
        "node_set_groups",
        json!({
            "scene_path": scene,
            "node_path": "Coin",
            "add": ["collectibles"]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 16. scene_read — verify hierarchy and positions
    let player = f.read_node(&scene, "Player");
    assert_eq!(player["type"], "CharacterBody2D");
    assert_approx(
        player["properties"]["position"]["x"].as_f64().unwrap(),
        64.0,
    );
    assert_approx(
        player["properties"]["position"]["y"].as_f64().unwrap(),
        192.0,
    );

    let hitbox = f.read_node(&scene, "Player/Hitbox");
    assert_eq!(hitbox["type"], "CollisionShape2D");

    let goomba = f.read_node(&scene, "Enemies/Goomba");
    assert_approx(
        goomba["properties"]["position"]["x"].as_f64().unwrap(),
        320.0,
    );
    assert_approx(
        goomba["properties"]["position"]["y"].as_f64().unwrap(),
        192.0,
    );

    let coin = f.read_node(&scene, "Coin");
    assert_eq!(coin["type"], "Area2D");
    assert!(coin["children"].as_array().is_some());

    // 17. node_find — enemies group → [Goomba]
    let enemies_data = f
        .run(
            "node_find",
            json!({"scene_path": scene, "group": "enemies"}),
        )
        .unwrap()
        .unwrap_data();
    let enemy_results = enemies_data["results"].as_array().unwrap();
    assert_eq!(enemy_results.len(), 1);
    assert_eq!(enemy_results[0]["name"], "Goomba");

    // 18. node_find — collectibles group → [Coin]
    let collectibles_data = f
        .run(
            "node_find",
            json!({"scene_path": scene, "group": "collectibles"}),
        )
        .unwrap()
        .unwrap_data();
    let collectible_results = collectibles_data["results"].as_array().unwrap();
    assert_eq!(collectible_results.len(), 1);
    assert_eq!(collectible_results[0]["name"], "Coin");

    // 19. scene_list — verify scene appears in listing
    let list = f
        .run("scene_list", json!({"directory": "tmp"}))
        .unwrap()
        .unwrap_data();
    let scenes = list["scenes"].as_array().unwrap();
    assert!(
        scenes.iter().any(|s| s["path"]
            .as_str()
            .unwrap_or("")
            .contains("j_platformer_level")),
        "Scene not found in listing"
    );

    // Verify Ground tilemap has >= 16 cells
    let ground_cells = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(ground_cells["cell_count"].as_u64().unwrap() >= 16);
}
