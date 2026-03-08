/// Wire tests for `execute_action`.
use crate::harness::{assert_approx, find_entity, GodotFixture};

fn snapshot(f: &mut GodotFixture) -> serde_json::Value {
    f.query(
        "get_snapshot_data",
        serde_json::json!({
            "perspective": {"type": "camera"},
            "radius": 200.0,
            "include_offscreen": true,
            "detail": "standard"
        }),
    )
    .unwrap()
    .unwrap_data()
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn pause_sets_tree_paused() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({ "action": "pause", "paused": true }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["action"], "pause");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn teleport_moves_node_and_returns_previous() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "teleport",
                "path": "TestScene3D/Enemies/Scout",
                "position": [10.0, 0.0, 0.0]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["action"], "teleport");

    // Previous position should be reported
    assert!(result["details"]["previous_position"].is_array());

    // Verify new position via snapshot
    let snap = snapshot(&mut f);
    let scout = find_entity(&snap, "Scout");
    assert_approx(scout["position"][0].as_f64().unwrap(), 10.0);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn set_property_changes_value_and_returns_previous() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "set_property",
                "path": "TestScene3D/Enemies/Scout",
                "property": "health",
                "value": 42
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["details"]["previous_value"], 80);
    assert_eq!(result["details"]["new_value"], 42);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn call_method_returns_result() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "call_method",
                "path": "TestScene3D",
                "method": "ping",
                "args": []
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["details"]["return_value"], "pong");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn action_on_missing_node_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "teleport",
                "path": "TestScene3D/DoesNotExist",
                "position": [0.0, 0.0, 0.0]
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn pause_and_advance_frames() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Pause
    f.query(
        "execute_action",
        serde_json::json!({ "action": "pause", "paused": true }),
    )
    .unwrap()
    .unwrap_data();

    // Get current frame
    let info1 = f
        .query("get_frame_info", serde_json::json!({}))
        .unwrap()
        .unwrap_data();
    let frame1 = info1["frame"].as_u64().unwrap();

    // Advance 5 frames (response is deferred — comes after physics ticks)
    let result = f
        .query(
            "execute_action",
            serde_json::json!({ "action": "advance_frames", "frames": 5 }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["action"], "advance_frames");
    assert_eq!(result["result"], "ok");

    let new_frame = result["details"]["new_frame"].as_u64().unwrap();
    assert_eq!(new_frame, frame1 + 5, "expected frame to advance by 5");
}
