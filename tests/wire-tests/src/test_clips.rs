/// Wire tests for dashcam clip queries.
use crate::harness::GodotFixture;

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn dashcam_status_returns_state() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let status = f
        .query("dashcam_status", serde_json::json!({}))
        .unwrap()
        .unwrap_data();

    assert!(
        status["state"].as_str().is_some(),
        "state should be present"
    );
    assert!(
        status["dashcam_enabled"].as_bool().is_some(),
        "dashcam_enabled should be present"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn recording_list_returns_clips_array() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query("recording_list", serde_json::json!({}))
        .unwrap()
        .unwrap_data();

    assert!(
        result["clips"].as_array().is_some(),
        "clips array should be present"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn recording_marker_triggers_dashcam_clip() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "recording_marker",
            serde_json::json!({
                "source": "agent",
                "label": "wire_test_marker"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["ok"], true, "marker should succeed");
    assert!(
        result["frame"].as_u64().is_some(),
        "frame should be present"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn dashcam_flush_returns_clip_id() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let result = f
        .query(
            "dashcam_flush",
            serde_json::json!({ "marker_label": "wire_test_save" }),
        )
        .unwrap()
        .unwrap_data();

    let clip_id = result["clip_id"].as_str().unwrap_or("");
    assert!(
        !clip_id.is_empty(),
        "clip_id should be non-empty on successful flush"
    );
    assert!(
        clip_id.starts_with("clip_"),
        "clip_id should start with 'clip_', got: {clip_id}"
    );
}
