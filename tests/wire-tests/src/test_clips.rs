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

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn configuration_rejects_bad_patches_and_marker_retains_post_window() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let applied = f
        .query(
            "dashcam_config",
            serde_json::json!({
                "enabled":true, "capture_interval":2, "pre_window_deliberate_sec":2,
                "post_window_deliberate_sec":1, "min_after_sec":0,
                "dense_burst_duration_sec":7, "anomaly_enabled":false,
                "screenshot_enabled":false
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(applied["result"], "ok");
    assert_eq!(applied["config"]["dense_burst_duration_sec"], 7);
    let before = applied["config"].clone();
    for patch in [
        serde_json::json!({"capture_interval":6, "pre_window_sec":{"deliberate":15}}),
        serde_json::json!({"capture_interval":6, "screenshot_quality":2.0}),
        serde_json::json!({"capture_interval":0}),
        serde_json::json!({"capture_interval":u64::MAX}),
    ] {
        assert!(f.query("dashcam_config", patch).unwrap().is_err());
        let status = f
            .query("dashcam_status", serde_json::json!({}))
            .unwrap()
            .unwrap_data();
        assert_eq!(
            status["config"], before,
            "invalid patches must not partially apply"
        );
    }
    let marker_label = format!("post-window coverage {:?}", std::time::SystemTime::now());
    let marker = f
        .query(
            "recording_marker",
            serde_json::json!({
                "source":"human", "label":marker_label
            }),
        )
        .unwrap()
        .unwrap_data();
    let trigger_frame = marker["frame"].as_u64().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        let status = f
            .query("dashcam_status", serde_json::json!({}))
            .unwrap()
            .unwrap_data();
        if status["state"] == "buffering" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "post-window did not finish: {status}"
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let clips = f
        .query("recording_list", serde_json::json!({}))
        .unwrap()
        .unwrap_data();
    let clip = clips["clips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|clip| clip["trigger_label"] == marker_label)
        .unwrap_or_else(|| panic!("marked clip missing; marker={marker}; clips={clips}"));
    let end = clip["frame_range"][1].as_u64().unwrap();
    let fps = f.handshake.physics_ticks_per_sec as u64;
    assert!(
        end >= trigger_frame + fps - 4,
        "post-window ended too early: {clip}"
    );
    assert!(
        end <= trigger_frame + fps + 8,
        "post-window ended too late: {clip}"
    );
    f.query(
        "recording_delete",
        serde_json::json!({"clip_id":clip["clip_id"]}),
    )
    .unwrap()
    .unwrap_data();
}
