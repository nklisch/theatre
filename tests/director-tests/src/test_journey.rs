use crate::harness::{assert_approx, DirectorFixture};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_create_scene_add_nodes_set_properties_read_back() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("journey_full");

    // 1. Create scene
    f.run("scene_create", json!({
        "scene_path": scene,
        "root_type": "Node2D"
    })).unwrap().unwrap_data();

    // 2. Add a player node
    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "CharacterBody2D",
        "node_name": "Player"
    })).unwrap().unwrap_data();

    // 3. Add a sprite to the player
    f.run("node_add", json!({
        "scene_path": scene,
        "parent_path": "Player",
        "node_type": "Sprite2D",
        "node_name": "Sprite"
    })).unwrap().unwrap_data();

    // 4. Add a collision shape to the player
    f.run("node_add", json!({
        "scene_path": scene,
        "parent_path": "Player",
        "node_type": "CollisionShape2D",
        "node_name": "Collision"
    })).unwrap().unwrap_data();

    // 5. Set position on the player
    f.run("node_set_properties", json!({
        "scene_path": scene,
        "node_path": "Player",
        "properties": {"position": {"x": 200, "y": 300}}
    })).unwrap().unwrap_data();

    // 6. Read back and verify full tree
    let data = f.run("scene_read", json!({
        "scene_path": scene
    })).unwrap().unwrap_data();

    let root = &data["root"];
    assert_eq!(root["type"], "Node2D");

    let player = &root["children"][0];
    assert_eq!(player["name"], "Player");
    assert_eq!(player["type"], "CharacterBody2D");
    assert_approx(player["properties"]["position"]["x"].as_f64().unwrap(), 200.0);
    assert_approx(player["properties"]["position"]["y"].as_f64().unwrap(), 300.0);

    let children = player["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["name"], "Sprite");
    assert_eq!(children[0]["type"], "Sprite2D");
    assert_eq!(children[1]["name"], "Collision");
    assert_eq!(children[1]["type"], "CollisionShape2D");

    // 7. Remove the sprite
    f.run("node_remove", json!({
        "scene_path": scene,
        "node_path": "Player/Sprite"
    })).unwrap().unwrap_data();

    // 8. Verify removal
    let data = f.run("scene_read", json!({
        "scene_path": scene
    })).unwrap().unwrap_data();
    let player = &data["root"]["children"][0];
    let children = player["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "Collision");
}
