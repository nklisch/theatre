use crate::harness::{DirectorFixture, project_dir_path};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn node_set_groups_add_and_remove() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("groups");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "Enemy"}),
    )
    .unwrap();

    let data = f
        .run(
            "node_set_groups",
            json!({
                "scene_path": &scene,
                "node_path": "Enemy",
                "add": ["enemies", "damageable"],
            }),
        )
        .unwrap()
        .unwrap_data();

    let groups = data["groups"].as_array().unwrap();
    assert!(groups.iter().any(|g| g == "enemies"));
    assert!(groups.iter().any(|g| g == "damageable"));

    // Remove one group
    let data2 = f
        .run(
            "node_set_groups",
            json!({
                "scene_path": &scene,
                "node_path": "Enemy",
                "remove": ["enemies"],
            }),
        )
        .unwrap()
        .unwrap_data();

    let groups2 = data2["groups"].as_array().unwrap();
    assert!(!groups2.iter().any(|g| g == "enemies"));
    assert!(groups2.iter().any(|g| g == "damageable"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_groups_no_filter_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("groups_no_filter");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();

    let result = f
        .run(
            "node_set_groups",
            json!({"scene_path": &scene, "node_path": "."}),
        )
        .unwrap();
    assert!(!result.success);
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_script_attach_and_detach() {
    use std::io::Write as IoWrite;
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("set_script");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "N"}),
    )
    .unwrap();

    // Create a minimal script file in the project
    let project_dir = project_dir_path();
    let script_dir = project_dir.join("scripts");
    std::fs::create_dir_all(&script_dir).unwrap();
    let script_path = script_dir.join("test_attach.gd");
    let mut file = std::fs::File::create(&script_path).unwrap();
    IoWrite::write_all(
        &mut file,
        b"extends Node2D\n\nfunc on_button_pressed():\n\tpass\n",
    )
    .unwrap();

    // Attach script
    let data = f
        .run(
            "node_set_script",
            json!({
                "scene_path": &scene,
                "node_path": "N",
                "script_path": "scripts/test_attach.gd",
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["node_path"], "N");
    assert!(
        data["script_path"]
            .as_str()
            .unwrap()
            .contains("test_attach.gd")
    );

    // Detach script
    let data2 = f
        .run(
            "node_set_script",
            json!({"scene_path": &scene, "node_path": "N"}),
        )
        .unwrap()
        .unwrap_data();

    assert!(data2["script_path"].is_null());
}

#[test]
#[ignore = "requires Godot binary"]
fn node_set_meta_set_and_remove() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("set_meta");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "N"}),
    )
    .unwrap();

    // Set metadata
    let data = f
        .run(
            "node_set_meta",
            json!({
                "scene_path": &scene,
                "node_path": "N",
                "meta": {"health": 100, "tag": "player"},
            }),
        )
        .unwrap()
        .unwrap_data();

    let keys = data["meta_keys"].as_array().unwrap();
    assert!(keys.iter().any(|k| k == "health"));
    assert!(keys.iter().any(|k| k == "tag"));

    // Remove one key
    let data2 = f
        .run(
            "node_set_meta",
            json!({
                "scene_path": &scene,
                "node_path": "N",
                "meta": {"health": null},
            }),
        )
        .unwrap()
        .unwrap_data();

    let keys2 = data2["meta_keys"].as_array().unwrap();
    assert!(!keys2.iter().any(|k| k == "health"));
    assert!(keys2.iter().any(|k| k == "tag"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_find_by_class() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("find_class");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Sprite2D", "node_name": "S1"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Sprite2D", "node_name": "S2"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "Other"}),
    )
    .unwrap();

    let data = f
        .run(
            "node_find",
            json!({"scene_path": &scene, "class_name": "Sprite2D"}),
        )
        .unwrap()
        .unwrap_data();

    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r["type"] == "Sprite2D"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_find_by_group() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("find_group");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "E1"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "E2"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "Other"}),
    )
    .unwrap();

    // Add E1 and E2 to "enemies" group
    f.run(
        "node_set_groups",
        json!({"scene_path": &scene, "node_path": "E1", "add": ["enemies"]}),
    )
    .unwrap();
    f.run(
        "node_set_groups",
        json!({"scene_path": &scene, "node_path": "E2", "add": ["enemies"]}),
    )
    .unwrap();

    let data = f
        .run(
            "node_find",
            json!({"scene_path": &scene, "group": "enemies"}),
        )
        .unwrap()
        .unwrap_data();

    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn node_find_combined_filters() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("find_combined");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Sprite2D", "node_name": "Enemy1"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Sprite2D", "node_name": "Player1"}),
    )
    .unwrap();

    let data = f
        .run(
            "node_find",
            json!({
                "scene_path": &scene,
                "class_name": "Sprite2D",
                "name_pattern": "Enemy*",
            }),
        )
        .unwrap()
        .unwrap_data();

    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "Enemy1");
}

#[test]
#[ignore = "requires Godot binary"]
fn node_find_no_filter_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("find_no_filter");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();

    let result = f.run("node_find", json!({"scene_path": &scene})).unwrap();
    assert!(!result.success);
}
