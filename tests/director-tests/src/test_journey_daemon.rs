use crate::harness::DaemonFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_daemon_multi_scene_workflow() {
    let mut d = DaemonFixture::start();

    let scene_a = "tmp/j_daemon_scene_a.tscn";
    let scene_b = "tmp/j_daemon_scene_b.tscn";

    // 2. Create Scene A
    d.run(
        "scene_create",
        json!({
            "scene_path": scene_a,
            "root_type": "Node2D",
            "root_name": "SceneA"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Add nodes to Scene A
    d.run(
        "node_add",
        json!({
            "scene_path": scene_a,
            "node_type": "Sprite2D",
            "node_name": "MainSprite"
        }),
    )
    .unwrap()
    .unwrap_data();
    d.run(
        "node_add",
        json!({
            "scene_path": scene_a,
            "node_type": "CollisionShape2D",
            "node_name": "Collision"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Set properties on Scene A
    d.run(
        "node_set_properties",
        json!({
            "scene_path": scene_a,
            "node_path": "MainSprite",
            "properties": {"position": {"x": 50, "y": 50}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Create Scene B
    d.run(
        "scene_create",
        json!({
            "scene_path": scene_b,
            "root_type": "Node2D",
            "root_name": "SceneB"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Instance Scene A into Scene B
    d.run(
        "scene_add_instance",
        json!({
            "scene_path": scene_b,
            "instance_scene": scene_a,
            "node_name": "SceneAInstance"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. scene_read — verify instance present in Scene B
    let scene_b_data = d
        .run("scene_read", json!({"scene_path": scene_b}))
        .unwrap()
        .unwrap_data();
    let root = &scene_b_data["root"];
    assert_eq!(root["type"], "Node2D");
    let children = root["children"].as_array().unwrap();
    assert_eq!(
        children.len(),
        1,
        "SceneB should have 1 child (the instance)"
    );
    assert_eq!(children[0]["name"], "SceneAInstance");

    // 8. scene_diff — diff A with B (structural comparison — different structures)
    let diff = d
        .run(
            "scene_diff",
            json!({"scene_a": scene_a, "scene_b": scene_b}),
        )
        .unwrap()
        .unwrap_data();
    // Just verify the operation succeeds and returns the expected fields
    assert!(diff.get("added").is_some());
    assert!(diff.get("removed").is_some());
    assert!(diff.get("changed").is_some());

    // 9. All operations succeeded via daemon — verified by no panics above

    // 10. Clean quit
    d.quit().unwrap();
}
