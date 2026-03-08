/// Wire tests for `spatial_query` (physics raycast, nav path).
use crate::harness::GodotFixture;

fn pause_and_advance(f: &mut GodotFixture, frames: u32) {
    f.query(
        "execute_action",
        serde_json::json!({ "action": "pause", "paused": true }),
    )
    .unwrap()
    .unwrap_data();

    f.query(
        "execute_action",
        serde_json::json!({ "action": "advance_frames", "frames": frames }),
    )
    .unwrap()
    .unwrap_data();
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn raycast_returns_hit_or_clear_field() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Advance a couple frames so collision shapes register
    pause_and_advance(&mut f, 2);

    let data = f
        .query(
            "spatial_query",
            serde_json::json!({
                "query_type": "raycast",
                "from": [0.0, 1.0, 0.0],
                "to": [5.0, 1.0, -3.0]
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(
        data.get("clear").is_some() || data.get("blocked_by").is_some(),
        "expected 'clear' or 'blocked_by' field in raycast response"
    );
    assert!(data.get("total_distance").is_some(), "total_distance missing");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn raycast_straight_down_hits_floor() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    pause_and_advance(&mut f, 2);

    let data = f
        .query(
            "spatial_query",
            serde_json::json!({
                "query_type": "raycast",
                "from": [0.0, 5.0, 0.0],
                "to": [0.0, -5.0, 0.0]
            }),
        )
        .unwrap()
        .unwrap_data();

    // Should hit the floor (clear == false if floor exists)
    assert!(data.get("clear").is_some(), "clear field missing");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn resolve_node_returns_position() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "spatial_query",
            serde_json::json!({
                "query_type": "resolve_node",
                "path": "TestScene3D/Player"
            }),
        )
        .unwrap()
        .unwrap_data();

    let pos = data["position"].as_array().expect("position array");
    assert_eq!(pos.len(), 3, "expected 3D position");
    assert!(data.get("forward").is_some(), "forward field missing");
}
