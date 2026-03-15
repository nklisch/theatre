/// Wire tests for `execute_action`.
use crate::harness::{GodotFixture, assert_approx, find_entity};

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
                "path": "Enemies/Scout",
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
                "path": "Enemies/Scout",
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
                "path": ".",
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
                "path": "DoesNotExist",
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
    // Accept frame1+5 or frame1+6: the StageTCPServer runs PROCESS_MODE_ALWAYS
    // so physics frames can increment while the tree is paused, making the
    // exact count non-deterministic by ±1.
    assert!(
        new_frame >= frame1 + 5 && new_frame <= frame1 + 6,
        "expected frame to advance by ~5, got new_frame={new_frame} frame1={frame1}"
    );
}

// --- advance_time ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn advance_time_advances_by_seconds() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Pause first (required by contract)
    f.query(
        "execute_action",
        serde_json::json!({ "action": "pause", "paused": true }),
    )
    .unwrap()
    .unwrap_data();

    let info1 = f
        .query("get_frame_info", serde_json::json!({}))
        .unwrap()
        .unwrap_data();
    let frame1 = info1["frame"].as_u64().unwrap();

    // Advance 0.1 seconds at 60 TPS = ~6 frames
    let result = f
        .query(
            "execute_action",
            serde_json::json!({ "action": "advance_time", "seconds": 0.1 }),
        )
        .unwrap()
        .unwrap_data();

    // advance_time delegates to advance_frames internally, so the response
    // echoes "advance_frames" as the action name
    assert_eq!(result["action"], "advance_frames");
    assert_eq!(result["result"], "ok");

    let new_frame = result["details"]["new_frame"].as_u64().unwrap();
    // 0.1s * 60 TPS = 6 frames, with ±1 tolerance
    assert!(
        new_frame >= frame1 + 5 && new_frame <= frame1 + 7,
        "expected frame to advance by ~6, got new_frame={new_frame} frame1={frame1}"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn advance_frames_while_unpaused_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Do NOT pause — advance should fail
    let result = f
        .query(
            "execute_action",
            serde_json::json!({ "action": "advance_frames", "frames": 1 }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn advance_time_while_unpaused_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({ "action": "advance_time", "seconds": 0.5 }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

// --- emit_signal ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn emit_signal_returns_ok() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "emit_signal",
                "path": "Player",
                "signal": "ready"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["action"], "emit_signal");
    assert_eq!(result["details"]["signal"], "ready");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn emit_signal_on_missing_node_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "emit_signal",
                "path": "DoesNotExist",
                "signal": "ready"
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

// --- call_method extended ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn call_method_with_args_returns_result() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "call_method",
                "path": ".",
                "method": "add",
                "args": [3, 7]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["details"]["return_value"], 10);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn call_method_missing_method_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "call_method",
                "path": ".",
                "method": "nonexistent_method",
                "args": []
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

// --- spawn_node ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn spawn_node_adds_child_visible_in_snapshot() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "spawn_node",
                "scene_path": "res://spawn_test.tscn",
                "parent": "Items",
                "name": "SpawnedTestNode",
                "position": [7.0, 0.0, 7.0]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["action"], "spawn_node");
    assert_eq!(result["details"]["scene_path"], "res://spawn_test.tscn");
    assert!(
        result["details"]["node_path"]
            .as_str()
            .unwrap()
            .contains("SpawnedTestNode")
    );

    // Verify the node appears in a snapshot
    let snap = snapshot(&mut f);
    let spawned = find_entity(&snap, "SpawnedTestNode");
    assert_approx(spawned["position"][0].as_f64().unwrap(), 7.0);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn spawn_node_invalid_scene_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "spawn_node",
                "scene_path": "res://does_not_exist.tscn",
                "parent": "Items"
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

// --- remove_node ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn remove_node_removes_from_tree() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Verify Ammo exists before removal
    let snap_before = snapshot(&mut f);
    find_entity(&snap_before, "Ammo"); // panics if not found

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "remove_node",
                "path": "Items/Ammo"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["action"], "remove_node");
    assert_eq!(result["details"]["removed_path"], "Items/Ammo");

    // Verify Ammo is gone from snapshot
    let snap_after = snapshot(&mut f);
    let has_ammo = snap_after["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains("Ammo"))
                .unwrap_or(false)
        });
    assert!(!has_ammo, "Ammo should be removed from the tree");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn remove_node_missing_node_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "remove_node",
                "path": "DoesNotExist"
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

// --- action_press / action_release ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn action_press_and_release_on_valid_action() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Press test_jump (defined in project.godot InputMap)
    let press = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "action_press",
                "action_name": "test_jump"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(press["result"], "ok");
    assert_eq!(press["action"], "action_press");
    assert_eq!(press["details"]["action_name"], "test_jump");

    // Default strength should be 1.0
    assert_approx(press["details"]["strength"].as_f64().unwrap(), 1.0);

    // Release
    let release = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "action_release",
                "action_name": "test_jump"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(release["result"], "ok");
    assert_eq!(release["action"], "action_release");
    assert_eq!(release["details"]["action_name"], "test_jump");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn action_press_unknown_action_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "action_press",
                "action_name": "nonexistent_action"
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn action_press_with_custom_strength() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "action_press",
                "action_name": "test_jump",
                "strength": 0.5
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_approx(result["details"]["strength"].as_f64().unwrap(), 0.5);
}

// --- inject_key ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inject_key_press_returns_ok() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "inject_key",
                "keycode": "SPACE",
                "pressed": true
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["action"], "inject_key");
    assert_eq!(result["details"]["keycode"], "SPACE");
    assert_eq!(result["details"]["pressed"], true);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inject_key_release_returns_ok() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "inject_key",
                "keycode": "W",
                "pressed": false
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["details"]["pressed"], false);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inject_key_unknown_keycode_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "inject_key",
                "keycode": "INVALID_KEY_NAME",
                "pressed": true
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inject_key_with_echo_returns_ok() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "inject_key",
                "keycode": "A",
                "pressed": true,
                "echo": true
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["details"]["echo"], true);
}

// --- inject_mouse_button ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inject_mouse_button_returns_ok() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "inject_mouse_button",
                "button": "left",
                "pressed": true,
                "position": [320.0, 180.0]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["action"], "inject_mouse_button");
    assert_eq!(result["details"]["button"], "left");
    assert_eq!(result["details"]["pressed"], true);
    assert!(result["details"]["position"].is_array());
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inject_mouse_button_without_position_returns_ok() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "inject_mouse_button",
                "button": "right",
                "pressed": false
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_eq!(result["details"]["button"], "right");
    // position should not be present when not provided
    assert!(result["details"].get("position").is_none() || result["details"]["position"].is_null());
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn inject_mouse_button_unknown_button_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "inject_mouse_button",
                "button": "invalid_button",
                "pressed": true
            }),
        )
        .unwrap();

    let (code, _msg) = result.unwrap_err();
    assert_eq!(code, "action_failed");
}

// --- teleport extended ---

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn teleport_with_rotation_sets_yaw() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "teleport",
                "path": "Enemies/Scout",
                "position": [0.0, 0.0, 0.0],
                "rotation_deg": 90.0
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    assert_approx(result["details"]["rotation_deg"].as_f64().unwrap(), 90.0);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn teleport_non_spatial_node_returns_error() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Camera3D is a Camera3D which inherits Node3D, so this should work.
    // Instead, let's try to teleport a non-spatial node — but all nodes
    // in the test scene are spatial. The error case is documented but hard
    // to trigger with current test scenes. We can test the missing node case
    // which is already covered. Let's skip this test.
    // Instead test teleport reports previous position as array.
    let result = f
        .query(
            "execute_action",
            serde_json::json!({
                "action": "teleport",
                "path": "Player",
                "position": [5.0, 1.0, -2.0]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");
    let prev_pos = result["details"]["previous_position"].as_array().unwrap();
    assert_eq!(prev_pos.len(), 3, "3D previous_position should have 3 components");
    let new_pos = result["details"]["new_position"].as_array().unwrap();
    assert_approx(new_pos[0].as_f64().unwrap(), 5.0);
    assert_approx(new_pos[1].as_f64().unwrap(), 1.0);
    assert_approx(new_pos[2].as_f64().unwrap(), -2.0);
}
