use crate::harness::{DirectorFixture, assert_approx, project_dir_path, OperationResultExt};
use serde_json::json;
use std::io::Write as IoWrite;

/// Capstone journey: create a complete small game scene using every major tool domain.
#[test]
#[ignore = "requires Godot binary"]
fn journey_full_game_scene_everything_together() {
    let f = DirectorFixture::new();
    let player_scene = DirectorFixture::journey_scene_path("full_player");
    let level_scene = DirectorFixture::journey_scene_path("full_level");
    let walk_anim = DirectorFixture::temp_resource_path("full_walk_anim");
    let player_mat = DirectorFixture::temp_resource_path("full_player_mat");

    // 1. Project setup: physics_set_layer_names for 2d_physics
    let layer_result = f
        .run(
            "physics_set_layer_names",
            json!({
                "layer_type": "2d_physics",
                "layers": {
                    "1": "player",
                    "2": "enemies",
                    "3": "environment"
                }
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(layer_result["layers_set"], 3);

    // 2. Player scene: scene_create + node_add
    f.run(
        "scene_create",
        json!({
            "scene_path": player_scene,
            "root_type": "CharacterBody2D",
            "root_name": "Player"
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": player_scene,
            "node_type": "Sprite2D",
            "node_name": "PlayerSprite"
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": player_scene,
            "node_type": "CollisionShape2D",
            "node_name": "PlayerCollision"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Shape: CapsuleShape2D attached to player collision
    f.run(
        "shape_create",
        json!({
            "shape_type": "CapsuleShape2D",
            "shape_params": {"radius": 16.0, "height": 48.0},
            "scene_path": player_scene,
            "node_path": "PlayerCollision"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Properties: node_set_properties position, node_set_groups "player"
    f.run(
        "node_set_properties",
        json!({
            "scene_path": player_scene,
            "node_path": ".",
            "properties": {"position": {"x": 0, "y": 0}}
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_set_groups",
        json!({
            "scene_path": player_scene,
            "node_path": ".",
            "add": ["player"]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Script: node_set_script attach a .gd file
    let project_dir = project_dir_path();
    let script_dir = project_dir.join("scripts");
    std::fs::create_dir_all(&script_dir).unwrap();
    let script_path = script_dir.join("journey_full_player.gd");
    let mut file = std::fs::File::create(&script_path).unwrap();
    IoWrite::write_all(
        &mut file,
        b"extends CharacterBody2D\n\nfunc _physics_process(_delta):\n\tpass\n",
    )
    .unwrap();

    let script_data = f
        .run(
            "node_set_script",
            json!({
                "scene_path": player_scene,
                "node_path": ".",
                "script_path": "scripts/journey_full_player.gd"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(
        script_data["script_path"]
            .as_str()
            .unwrap()
            .contains("journey_full_player.gd")
    );

    // 6. Meta: node_set_meta set editor metadata
    f.run(
        "node_set_meta",
        json!({
            "scene_path": player_scene,
            "node_path": ".",
            "meta": {"author": "agent", "version": 1}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. Material: material_create StandardMaterial3D
    f.run(
        "material_create",
        json!({
            "resource_path": player_mat,
            "material_type": "StandardMaterial3D",
            "properties": {"albedo_color": {"r": 0.2, "g": 0.6, "b": 1.0, "a": 1.0}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 8. Animation: animation_create + animation_add_track walk animation
    f.run(
        "animation_create",
        json!({
            "resource_path": walk_anim,
            "length": 0.6,
            "loop_mode": "linear"
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "animation_add_track",
        json!({
            "resource_path": walk_anim,
            "track_type": "value",
            "node_path": "PlayerSprite:frame",
            "update_mode": "discrete",
            "keyframes": [
                {"time": 0.0, "value": 0},
                {"time": 0.2, "value": 1},
                {"time": 0.4, "value": 2}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // Verify animation track
    let anim_read = f
        .run("animation_read", json!({"resource_path": walk_anim}))
        .unwrap()
        .unwrap_data();
    assert_eq!(anim_read["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(anim_read["loop_mode"], "linear");

    // 9. Level scene: scene_create + node_add structure
    f.run(
        "scene_create",
        json!({
            "scene_path": level_scene,
            "root_type": "Node2D",
            "root_name": "Level"
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({"scene_path": level_scene, "node_type": "Node2D", "node_name": "Entities"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": level_scene, "node_type": "Node2D", "node_name": "Environment"}),
    )
    .unwrap()
    .unwrap_data();

    // 10. TileMap: Add TileMapLayer + tilemap_set_cells
    f.run(
        "node_add",
        json!({
            "scene_path": level_scene,
            "parent_path": "Environment",
            "node_type": "TileMapLayer",
            "node_name": "Ground",
            "properties": {"tile_set": "res://fixtures/test_tileset.tres"}
        }),
    )
    .unwrap()
    .unwrap_data();

    let tiles: Vec<serde_json::Value> = (0..10)
        .map(|x| json!({"coords": [x, 0], "source_id": 0, "atlas_coords": [0, 0]}))
        .collect();
    let tiles_data = f
        .run(
            "tilemap_set_cells",
            json!({
                "scene_path": level_scene,
                "node_path": "Environment/Ground",
                "cells": tiles
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(tiles_data["cells_set"], 10);

    // 11. Instance: scene_add_instance player into level
    f.run(
        "scene_add_instance",
        json!({
            "scene_path": level_scene,
            "instance_scene": player_scene,
            "parent_path": "Entities",
            "node_name": "PlayerInstance"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 12. Physics: physics_set_layers on entities in level
    f.run(
        "node_add",
        json!({
            "scene_path": level_scene,
            "parent_path": "Environment",
            "node_type": "StaticBody2D",
            "node_name": "Wall"
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "physics_set_layers",
        json!({
            "scene_path": level_scene,
            "node_path": "Environment/Wall",
            "collision_layer": 4,
            "collision_mask": 0
        }),
    )
    .unwrap()
    .unwrap_data();

    // 14. Batch: bulk node additions to level
    let batch_result = f
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "node_add", "params": {
                        "scene_path": level_scene,
                        "parent_path": "Entities",
                        "node_type": "CharacterBody2D",
                        "node_name": "Enemy1"
                    }},
                    {"operation": "node_set_properties", "params": {
                        "scene_path": level_scene,
                        "node_path": "Entities/Enemy1",
                        "properties": {"position": {"x": 400, "y": 0}}
                    }},
                    {"operation": "node_set_groups", "params": {
                        "scene_path": level_scene,
                        "node_path": "Entities/Enemy1",
                        "add": ["enemies"]
                    }}
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(batch_result["completed"], 3);
    assert_eq!(batch_result["failed"], 0);

    // 15. Diff: scene_diff level against itself (sanity check)
    let diff = f
        .run(
            "scene_diff",
            json!({"scene_a": level_scene, "scene_b": level_scene}),
        )
        .unwrap()
        .unwrap_data();
    assert!(diff["added"].as_array().unwrap().is_empty());
    assert!(diff["removed"].as_array().unwrap().is_empty());
    assert!(diff["changed"].as_array().unwrap().is_empty());

    // 16. Read: scene_read full level tree
    let level_root = f.read_node(&level_scene, ".");
    assert_eq!(level_root["type"], "Node2D");

    let entities = f.read_node(&level_scene, "Entities");
    assert_eq!(entities["type"], "Node2D");
    let entity_children = entities["children"].as_array().unwrap();
    assert!(
        entity_children
            .iter()
            .any(|c| c["name"] == "PlayerInstance")
    );
    assert!(entity_children.iter().any(|c| c["name"] == "Enemy1"));

    let ground = f.read_node(&level_scene, "Environment/Ground");
    assert_eq!(ground["type"], "TileMapLayer");

    // 17. List: scene_list verify all created scenes
    let list = f
        .run("scene_list", json!({"directory": "tmp"}))
        .unwrap()
        .unwrap_data();
    let scenes = list["scenes"].as_array().unwrap();
    assert!(
        scenes
            .iter()
            .any(|s| s["path"].as_str().unwrap_or("").contains("full_player"))
    );
    assert!(
        scenes
            .iter()
            .any(|s| s["path"].as_str().unwrap_or("").contains("full_level"))
    );

    // 18. Find: node_find verify searchability
    let found_enemies = f
        .run(
            "node_find",
            json!({
                "scene_path": level_scene,
                "group": "enemies"
            }),
        )
        .unwrap()
        .unwrap_data();
    let found = found_enemies["results"].as_array().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["name"], "Enemy1");

    // 19. UID: uid_update_project scans and registers UIDs
    let uid_scan = f
        .run("uid_update_project", json!({"directory": "tmp"}))
        .unwrap()
        .unwrap_data();
    assert!(uid_scan["files_scanned"].as_u64().unwrap() > 0);

    // uid_get may not find UIDs in one-shot mode (each invocation is a separate
    // Godot process, so in-memory UID registration doesn't persist). Test that
    // uid_get runs without crashing and returns a well-formed response.
    let uid_result = f
        .run("uid_get", json!({"file_path": level_scene}))
        .unwrap();
    if uid_result.success {
        let uid_str = uid_result.data["uid"].as_str().unwrap();
        assert!(
            uid_str.starts_with("uid://"),
            "Level UID should start with uid://, got: {uid_str}"
        );
    }

    // Verify player material was created
    let mat_read = f
        .run("resource_read", json!({"resource_path": player_mat}))
        .unwrap()
        .unwrap_data();
    assert_eq!(mat_read["type"], "StandardMaterial3D");
    assert_approx(
        mat_read["properties"]["albedo_color"]["g"]
            .as_f64()
            .unwrap(),
        0.6,
    );
}
