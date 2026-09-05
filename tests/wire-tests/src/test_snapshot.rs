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
        !entities.is_empty(),
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
fn exported_state_preserves_inherited_dynamic_and_validated_properties() {
    let mut f = GodotFixture::start("tests/exported_state_journey.gd").unwrap();
    for changed in [false, true] {
        if changed {
            f.query(
                "execute_action",
                serde_json::json!({
                    "action": "call_method", "path": "Subject",
                    "method": "change_exports", "args": []
                }),
            )
            .unwrap()
            .unwrap_data();
        }
        let expected = if changed {
            serde_json::json!({"health":17, "inventory":["gem"], "direction":[-1.0,8.0],
                "visible":false, "dynamic_after":456, "hidden_export":7})
        } else {
            serde_json::json!({"health":42, "inventory":["key","coin"], "direction":[2.0,3.0],
                "visible":true, "dynamic_before":123})
        };
        for detail in ["summary", "standard", "full"] {
            let data = f
                .query(
                    "get_snapshot_data",
                    serde_json::json!({
                        "perspective":{"type":"camera"}, "detail":detail,
                        "radius":200.0, "include_offscreen":true
                    }),
                )
                .unwrap()
                .unwrap_data();
            let subject = find_entity(&data, "Subject");
            assert_eq!(subject["path"], "Subject");
            assert_eq!(subject["state"], expected, "{detail}, changed={changed}");
            if detail == "full" {
                assert_eq!(subject["all_exported_vars"], expected);
            }
        }
        let inspected = f
            .query(
                "get_node_inspect",
                serde_json::json!({
                    "path":"Subject", "include":["state"]
                }),
            )
            .unwrap()
            .unwrap_data();
        assert_eq!(inspected["state"]["exported"], expected);
    }
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
