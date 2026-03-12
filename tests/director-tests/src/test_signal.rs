use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn signal_connect_basic() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_connect");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Button", "node_name": "MyButton"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "Handler"}),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "signal_connect",
            json!({
                "scene_path": &scene,
                "source_path": "MyButton",
                "signal_name": "pressed",
                "target_path": "Handler",
                "method_name": "on_button_pressed",
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["signal_name"], "pressed");
    assert_eq!(data["source_path"], "MyButton");
    assert_eq!(data["target_path"], "Handler");
    assert_eq!(data["method_name"], "on_button_pressed");

    // Verify via signal_list
    let list = f
        .run("signal_list", json!({"scene_path": &scene}))
        .unwrap()
        .unwrap_data();
    let connections = list["connections"].as_array().unwrap();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0]["signal_name"], "pressed");
    assert_eq!(connections[0]["method_name"], "on_button_pressed");
}

#[test]
#[ignore = "requires Godot binary"]
fn signal_disconnect_removes_connection() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_disconnect");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Button", "node_name": "Btn"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "H"}),
    )
    .unwrap();

    f.run(
        "signal_connect",
        json!({
            "scene_path": &scene,
            "source_path": "Btn",
            "signal_name": "pressed",
            "target_path": "H",
            "method_name": "on_press",
        }),
    )
    .unwrap()
    .unwrap_data();

    // Verify connected
    let list_before = f
        .run("signal_list", json!({"scene_path": &scene}))
        .unwrap()
        .unwrap_data();
    assert_eq!(list_before["connections"].as_array().unwrap().len(), 1);

    // Disconnect
    f.run(
        "signal_disconnect",
        json!({
            "scene_path": &scene,
            "source_path": "Btn",
            "signal_name": "pressed",
            "target_path": "H",
            "method_name": "on_press",
        }),
    )
    .unwrap()
    .unwrap_data();

    // Verify disconnected
    let list_after = f
        .run("signal_list", json!({"scene_path": &scene}))
        .unwrap()
        .unwrap_data();
    assert_eq!(list_after["connections"].as_array().unwrap().len(), 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn signal_connect_invalid_signal_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_invalid");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "A"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "B"}),
    )
    .unwrap();

    let result = f
        .run(
            "signal_connect",
            json!({
                "scene_path": &scene,
                "source_path": "A",
                "signal_name": "nonexistent_signal",
                "target_path": "B",
                "method_name": "some_method",
            }),
        )
        .unwrap();

    assert!(!result.success);
}

#[test]
#[ignore = "requires Godot binary"]
fn signal_list_empty_scene() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_list_empty");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();

    let data = f
        .run("signal_list", json!({"scene_path": &scene}))
        .unwrap()
        .unwrap_data();
    let connections = data["connections"].as_array().unwrap();
    assert_eq!(connections.len(), 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn signal_list_filtered_by_node() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("signal_list_filter");

    f.run(
        "scene_create",
        json!({"scene_path": &scene, "root_type": "Node2D"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Button", "node_name": "Btn1"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Button", "node_name": "Btn2"}),
    )
    .unwrap();
    f.run(
        "node_add",
        json!({"scene_path": &scene, "parent_path": ".", "node_type": "Node2D", "node_name": "H"}),
    )
    .unwrap();

    f.run(
        "signal_connect",
        json!({
            "scene_path": &scene, "source_path": "Btn1",
            "signal_name": "pressed", "target_path": "H", "method_name": "on1",
        }),
    )
    .unwrap();
    f.run(
        "signal_connect",
        json!({
            "scene_path": &scene, "source_path": "Btn2",
            "signal_name": "pressed", "target_path": "H", "method_name": "on2",
        }),
    )
    .unwrap();

    // Filter to Btn1 only
    let data = f
        .run(
            "signal_list",
            json!({"scene_path": &scene, "node_path": "Btn1"}),
        )
        .unwrap()
        .unwrap_data();
    let connections = data["connections"].as_array().unwrap();
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0]["source_path"], "Btn1");

    // Filter to H — should see both (as target)
    let data2 = f
        .run(
            "signal_list",
            json!({"scene_path": &scene, "node_path": "H"}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data2["connections"].as_array().unwrap().len(), 2);
}
