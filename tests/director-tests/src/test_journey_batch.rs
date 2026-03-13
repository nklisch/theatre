use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_batch_create_complete_scene() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("batch_complete");

    // 1. Create the scene first (needed before batch)
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. Single batch with 7 operations
    let batch_data = f
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "CharacterBody2D",
                        "node_name": "Player"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "parent_path": "Player",
                        "node_type": "Sprite2D",
                        "node_name": "PlayerSprite"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "parent_path": "Player",
                        "node_type": "CollisionShape2D",
                        "node_name": "Hitbox"
                    }},
                    {"operation": "node_set_properties", "params": {
                        "scene_path": scene,
                        "node_path": "Player",
                        "properties": {"position": {"x": 100, "y": 200}}
                    }},
                    {"operation": "node_set_groups", "params": {
                        "scene_path": scene,
                        "node_path": "Player",
                        "add": ["player"]
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "Area2D",
                        "node_name": "DamageZone"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "parent_path": "DamageZone",
                        "node_type": "CollisionShape2D",
                        "node_name": "DZShape"
                    }}
                ]
            }),
        )
        .unwrap()
        .unwrap_data();

    // 3. Verify batch results: completed=7, failed=0
    assert_eq!(
        batch_data["completed"], 7,
        "All 7 batch operations should succeed"
    );
    assert_eq!(batch_data["failed"], 0, "No batch operations should fail");

    // 4. scene_read — verify complete tree
    let player = f.read_node(&scene, "Player");
    assert_eq!(player["type"], "CharacterBody2D");

    let player_children = player["children"].as_array().unwrap();
    assert_eq!(player_children.len(), 2);
    assert!(player_children.iter().any(|c| c["name"] == "PlayerSprite"));
    assert!(player_children.iter().any(|c| c["name"] == "Hitbox"));

    let dz = f.read_node(&scene, "DamageZone");
    assert_eq!(dz["type"], "Area2D");
    let dz_children = dz["children"].as_array().unwrap();
    assert_eq!(dz_children.len(), 1);
    assert_eq!(dz_children[0]["name"], "DZShape");

    // 5. node_find — verify group membership set during batch
    let player_group = f
        .run("node_find", json!({"scene_path": scene, "group": "player"}))
        .unwrap()
        .unwrap_data();
    let group_results = player_group["results"].as_array().unwrap();
    assert_eq!(group_results.len(), 1);
    assert_eq!(group_results[0]["name"], "Player");
}

#[test]
#[ignore = "requires Godot binary"]
fn journey_batch_partial_failure_recovery() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("batch_partial");

    // 1. Create scene
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. Batch with stop_on_error=false — one bad operation in the middle
    // Note: batch returns success=false when any sub-op fails, but data is still populated
    let batch_result = f
        .run(
            "batch",
            json!({
                "stop_on_error": false,
                "operations": [
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "Node2D",
                        "node_name": "NodeA"
                    }},
                    {"operation": "node_set_properties", "params": {
                        "scene_path": scene,
                        "node_path": "NodeA",
                        "properties": {"nonexistent_property_xyz": 42}
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "Node2D",
                        "node_name": "NodeB"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "Node2D",
                        "node_name": "NodeC"
                    }}
                ]
            }),
        )
        .unwrap();
    let batch_data = &batch_result.data;

    // 3. Verify: completed=3, failed=1
    assert_eq!(batch_data["completed"], 3, "3 operations should succeed");
    assert_eq!(
        batch_data["failed"], 1,
        "1 operation should fail (bad property)"
    );

    // 4. scene_read — verify A, B, C all exist despite the mid-batch failure
    let root_node = f.read_node(&scene, ".");
    let children = root_node["children"].as_array().unwrap();
    assert!(
        children.iter().any(|c| c["name"] == "NodeA"),
        "NodeA should exist"
    );
    assert!(
        children.iter().any(|c| c["name"] == "NodeB"),
        "NodeB should exist"
    );
    assert!(
        children.iter().any(|c| c["name"] == "NodeC"),
        "NodeC should exist"
    );
}
