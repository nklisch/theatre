/// Wire tests for `get_snapshot_data`.
use crate::harness::{GodotFixture, assert_approx, find_entity};

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn snapshot_returns_entities_with_positions() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_snapshot_data",
            serde_json::json!({
                "perspective": {"type": "camera"},
                "radius": 200.0,
                "include_offscreen": true,
                "detail": "standard"
            }),
        )
        .unwrap()
        .unwrap_data();

    let entities = data["entities"].as_array().expect("entities array");
    assert!(
        entities.len() >= 1,
        "expected at least 1 entity, got {}",
        entities.len()
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn snapshot_player_at_origin() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_snapshot_data",
            serde_json::json!({
                "perspective": {"type": "camera"},
                "radius": 200.0,
                "include_offscreen": true,
                "detail": "standard"
            }),
        )
        .unwrap()
        .unwrap_data();

    let player = find_entity(&data, "Player");
    let pos = &player["position"];
    assert_approx(pos[0].as_f64().unwrap(), 0.0);
    assert_approx(pos[1].as_f64().unwrap(), 0.0);
    assert_approx(pos[2].as_f64().unwrap(), 0.0);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn snapshot_includes_groups() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_snapshot_data",
            serde_json::json!({
                "perspective": {"type": "camera"},
                "radius": 200.0,
                "include_offscreen": true,
                "detail": "standard"
            }),
        )
        .unwrap()
        .unwrap_data();

    let scout = find_entity(&data, "Scout");
    let groups = scout["groups"].as_array().expect("groups array");
    assert!(
        groups.iter().any(|g| g == "enemies"),
        "expected Scout to be in 'enemies' group"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn snapshot_includes_state_exports_at_full_detail() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_snapshot_data",
            serde_json::json!({
                "perspective": {"type": "camera"},
                "radius": 200.0,
                "include_offscreen": true,
                "detail": "full"
            }),
        )
        .unwrap()
        .unwrap_data();

    let scout = find_entity(&data, "Scout");
    assert_eq!(scout["state"]["health"], 80, "Scout health should be 80");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn snapshot_2d_has_2_component_positions() {
    let mut f = GodotFixture::start("test_scene_2d.tscn").unwrap();

    let data = f
        .query(
            "get_snapshot_data",
            serde_json::json!({
                "perspective": {"type": "camera"},
                "radius": 200.0,
                "include_offscreen": true,
                "detail": "standard"
            }),
        )
        .unwrap()
        .unwrap_data();

    let player = find_entity(&data, "Player");
    let pos = player["position"].as_array().expect("position array");
    assert_eq!(pos.len(), 2, "2D position should have 2 components");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn snapshot_response_has_frame_and_timestamp() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let data = f
        .query(
            "get_snapshot_data",
            serde_json::json!({
                "perspective": {"type": "camera"},
                "radius": 200.0,
                "include_offscreen": true,
                "detail": "summary"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(data.get("frame").is_some(), "frame field missing");
    assert!(
        data.get("timestamp_ms").is_some(),
        "timestamp_ms field missing"
    );
}
