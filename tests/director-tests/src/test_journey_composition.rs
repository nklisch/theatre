use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_compose_game_from_reusable_scenes() {
    let f = DirectorFixture::new();
    let enemy_scene = DirectorFixture::journey_scene_path("composition_enemy");
    let health_pack_scene = DirectorFixture::journey_scene_path("composition_health_pack");
    let level_scene = DirectorFixture::journey_scene_path("composition_level");

    // 1. Create "enemy.tscn" with CharacterBody2D root
    f.run(
        "scene_create",
        json!({
            "scene_path": enemy_scene,
            "root_type": "CharacterBody2D",
            "root_name": "Enemy"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 2. Add Sprite2D + CollisionShape2D under enemy root
    f.run(
        "node_add",
        json!({
            "scene_path": enemy_scene,
            "node_type": "Sprite2D",
            "node_name": "EnemySprite"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": enemy_scene,
            "node_type": "CollisionShape2D",
            "node_name": "EnemyCollision"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Set enemy root properties
    f.run(
        "node_set_properties",
        json!({
            "scene_path": enemy_scene,
            "node_path": ".",
            "properties": {"position": {"x": 0, "y": 0}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Create "health_pack.tscn" with Area2D root
    f.run(
        "scene_create",
        json!({
            "scene_path": health_pack_scene,
            "root_type": "Area2D",
            "root_name": "HealthPack"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Add CollisionShape2D under health_pack root
    f.run(
        "node_add",
        json!({
            "scene_path": health_pack_scene,
            "node_type": "CollisionShape2D",
            "node_name": "PickupArea"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Create "level.tscn" with Node2D root
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

    // 7. Add container nodes
    f.run(
        "node_add",
        json!({"scene_path": level_scene, "node_type": "Node2D", "node_name": "Enemies"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": level_scene, "node_type": "Node2D", "node_name": "Items"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": level_scene, "node_type": "Node2D", "node_name": "Environment"}),
    )
    .unwrap()
    .unwrap_data();

    // 8. Instance enemy.tscn as "Enemy1" under Enemies
    f.run(
        "scene_add_instance",
        json!({
            "scene_path": level_scene,
            "instance_scene": enemy_scene,
            "parent_path": "Enemies",
            "node_name": "Enemy1"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 9. Instance enemy.tscn as "Enemy2" under Enemies
    f.run(
        "scene_add_instance",
        json!({
            "scene_path": level_scene,
            "instance_scene": enemy_scene,
            "parent_path": "Enemies",
            "node_name": "Enemy2"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 10. Instance health_pack.tscn as "HealthPack1" under Items
    f.run(
        "scene_add_instance",
        json!({
            "scene_path": level_scene,
            "instance_scene": health_pack_scene,
            "parent_path": "Items",
            "node_name": "HealthPack1"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 11. Set Enemy1 position to (100, 0)
    f.run(
        "node_set_properties",
        json!({
            "scene_path": level_scene,
            "node_path": "Enemies/Enemy1",
            "properties": {"position": {"x": 100, "y": 0}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 12. Set Enemy2 position to (300, 0)
    f.run(
        "node_set_properties",
        json!({
            "scene_path": level_scene,
            "node_path": "Enemies/Enemy2",
            "properties": {"position": {"x": 300, "y": 0}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 13. Set HealthPack1 position to (200, -50)
    f.run(
        "node_set_properties",
        json!({
            "scene_path": level_scene,
            "node_path": "Items/HealthPack1",
            "properties": {"position": {"x": 200, "y": -50}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 14. scene_read — verify instances are present with correct names
    let enemies_node = f.read_node(&level_scene, "Enemies");
    let enemy_children = enemies_node["children"].as_array().unwrap();
    assert_eq!(enemy_children.len(), 2);
    assert!(enemy_children.iter().any(|c| c["name"] == "Enemy1"));
    assert!(enemy_children.iter().any(|c| c["name"] == "Enemy2"));

    let items_node = f.read_node(&level_scene, "Items");
    let item_children = items_node["children"].as_array().unwrap();
    assert_eq!(item_children.len(), 1);
    assert_eq!(item_children[0]["name"], "HealthPack1");

    // 15. node_reparent — Move Enemy1 from Enemies to Environment
    let reparent_data = f
        .run(
            "node_reparent",
            json!({
                "scene_path": level_scene,
                "node_path": "Enemies/Enemy1",
                "new_parent_path": "Environment"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(reparent_data["old_path"], "Enemies/Enemy1");
    assert_eq!(reparent_data["new_path"], "Environment/Enemy1");

    // 16. scene_read — verify Enemy1 is now under Environment
    let env_node = f.read_node(&level_scene, "Environment");
    let env_children = env_node["children"].as_array().unwrap();
    assert_eq!(env_children.len(), 1);
    assert_eq!(env_children[0]["name"], "Enemy1");

    // Enemies now has only Enemy2
    let enemies_after = f.read_node(&level_scene, "Enemies");
    let enemies_children = enemies_after["children"].as_array().unwrap();
    assert_eq!(enemies_children.len(), 1);
    assert_eq!(enemies_children[0]["name"], "Enemy2");

    // 17. scene_diff — diff level against itself (no spurious changes)
    let diff_data = f
        .run(
            "scene_diff",
            json!({"scene_a": level_scene, "scene_b": level_scene}),
        )
        .unwrap()
        .unwrap_data();
    assert!(diff_data["added"].as_array().unwrap().is_empty());
    assert!(diff_data["removed"].as_array().unwrap().is_empty());
    assert!(diff_data["changed"].as_array().unwrap().is_empty());

    // 18. scene_list — verify all 3 scenes appear
    let list = f
        .run("scene_list", json!({"directory": "tmp"}))
        .unwrap()
        .unwrap_data();
    let scenes = list["scenes"].as_array().unwrap();
    assert!(scenes.iter().any(|s| {
        s["path"]
            .as_str()
            .unwrap_or("")
            .contains("composition_enemy")
    }));
    assert!(scenes.iter().any(|s| {
        s["path"]
            .as_str()
            .unwrap_or("")
            .contains("composition_health_pack")
    }));
    assert!(scenes.iter().any(|s| {
        s["path"]
            .as_str()
            .unwrap_or("")
            .contains("composition_level")
    }));

    // Verify enemy scene root type
    let enemy_scene_entry = scenes
        .iter()
        .find(|s| {
            s["path"]
                .as_str()
                .unwrap_or("")
                .contains("composition_enemy")
        })
        .unwrap();
    assert_eq!(enemy_scene_entry["root_type"], "CharacterBody2D");

    let health_scene_entry = scenes
        .iter()
        .find(|s| {
            s["path"]
                .as_str()
                .unwrap_or("")
                .contains("composition_health_pack")
        })
        .unwrap();
    assert_eq!(health_scene_entry["root_type"], "Area2D");
}
