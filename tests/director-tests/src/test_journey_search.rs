use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_node_find_complex_search() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("search_complex");

    // 1. Create scene with diverse node tree
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. Add multiple Sprite2D, CharacterBody2D, Area2D nodes at various depths
    // Sprites
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Sprite2D",
            "node_name": "PlayerSprite"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Sprite2D",
            "node_name": "EnemySprite1"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Sprite2D",
            "node_name": "EnemySprite2"
        }),
    )
    .unwrap()
    .unwrap_data();

    // CharacterBody2D nodes
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "CharacterBody2D",
            "node_name": "EnemyBody1"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "CharacterBody2D",
            "node_name": "EnemyBody2"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "CharacterBody2D",
            "node_name": "PlayerBody"
        }),
    )
    .unwrap()
    .unwrap_data();

    // Area2D nodes
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Area2D",
            "node_name": "Item1"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Area2D",
            "node_name": "Item2"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Add some to "enemies" and "items" groups
    f.run(
        "node_set_groups",
        json!({
            "scene_path": scene,
            "node_path": "EnemyBody1",
            "add": ["enemies"]
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_set_groups",
        json!({
            "scene_path": scene,
            "node_path": "EnemyBody2",
            "add": ["enemies"]
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_set_groups",
        json!({
            "scene_path": scene,
            "node_path": "Item1",
            "add": ["items"]
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_set_groups",
        json!({
            "scene_path": scene,
            "node_path": "Item2",
            "add": ["items"]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. node_find — class_name="Sprite2D" → all 3 sprites
    let sprites = f
        .run(
            "node_find",
            json!({
                "scene_path": scene,
                "class_name": "Sprite2D"
            }),
        )
        .unwrap()
        .unwrap_data();
    let sprite_results = sprites["results"].as_array().unwrap();
    assert_eq!(sprite_results.len(), 3, "Should find 3 Sprite2D nodes");
    assert!(sprite_results.iter().all(|r| r["type"] == "Sprite2D"));

    // 6. node_find — group="enemies" → only enemy nodes
    let enemies = f
        .run(
            "node_find",
            json!({
                "scene_path": scene,
                "group": "enemies"
            }),
        )
        .unwrap()
        .unwrap_data();
    let enemy_results = enemies["results"].as_array().unwrap();
    assert_eq!(enemy_results.len(), 2, "Should find 2 enemy nodes");
    assert!(
        enemy_results
            .iter()
            .all(|r| r["name"].as_str().unwrap_or("").starts_with("Enemy"))
    );

    // 7. node_find — name_pattern="Enemy*" → all Enemy-named nodes
    let enemy_named = f
        .run(
            "node_find",
            json!({
                "scene_path": scene,
                "name_pattern": "Enemy*"
            }),
        )
        .unwrap()
        .unwrap_data();
    let enemy_named_results = enemy_named["results"].as_array().unwrap();
    // Expect 4: EnemySprite1, EnemySprite2, EnemyBody1, EnemyBody2
    assert_eq!(
        enemy_named_results.len(),
        4,
        "Should find 4 Enemy-named nodes"
    );
    assert!(
        enemy_named_results
            .iter()
            .all(|r| { r["name"].as_str().unwrap_or("").starts_with("Enemy") })
    );

    // 8. node_find — Combined: class_name="CharacterBody2D" AND group="enemies"
    let enemy_bodies = f
        .run(
            "node_find",
            json!({
                "scene_path": scene,
                "class_name": "CharacterBody2D",
                "group": "enemies"
            }),
        )
        .unwrap()
        .unwrap_data();
    let enemy_body_results = enemy_bodies["results"].as_array().unwrap();
    assert_eq!(
        enemy_body_results.len(),
        2,
        "Should find 2 enemy CharacterBody2D nodes"
    );
    assert!(
        enemy_body_results
            .iter()
            .all(|r| r["type"] == "CharacterBody2D")
    );
    assert!(
        enemy_body_results
            .iter()
            .all(|r| { r["name"].as_str().unwrap_or("").starts_with("EnemyBody") })
    );
}
