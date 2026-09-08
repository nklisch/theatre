/// Wire coverage for explicit capture, saved evidence, and staged settings.
use crate::harness::GodotFixture;
use serde_json::json;

fn control(f: &mut GodotFixture, action: &str) -> serde_json::Value {
    f.query("capture_control", json!({"action":action}))
        .unwrap()
        .unwrap_data()
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn capture_status_reports_explicit_launch_state() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let status = control(&mut f, "status");
    assert_eq!(status["state"], "recording");
    assert_eq!(status["launch"]["observe"], true);
    assert_eq!(status["config"]["preset"], "heavy");
    assert_eq!(status["config"]["images"], false);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn recording_list_returns_clips_array() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    assert!(f.query("recording_list", json!({})).unwrap().unwrap_data()["clips"].is_array());
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn manual_marker_annotates_without_implicit_save() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let before = control(&mut f, "status");
    let marker = f
        .query(
            "recording_marker",
            json!({"source":"agent","label":"wire marker"}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(marker["ok"], true);
    assert!(marker["frame"].is_u64());
    let after = control(&mut f, "status");
    assert_eq!(after["state"], "recording");
    assert_eq!(after["last_saved_clip"], before["last_saved_clip"]);
    assert_eq!(after["pending_post_window"], false);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn save_stops_and_keeps_then_continue_creates_a_separate_segment() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let saved = control(&mut f, "save");
    let clip = &saved["last_saved_clip"];
    assert!(clip["clip_id"].as_str().unwrap().starts_with("clip_"));
    assert_eq!(saved["state"], "idle");
    let continued = control(&mut f, "continue");
    assert_eq!(continued["recording_id"], clip["recording_id"]);
    assert_ne!(continued["segment_id"], clip["clip_id"]);
    f.query("recording_delete", json!({"clip_id":clip["clip_id"]}))
        .unwrap()
        .unwrap_data();
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn configuration_is_staged_and_explicit_trigger_retains_post_window() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let original = control(&mut f, "status");
    let applied = f
        .query(
            "capture_control",
            json!({"action":"config","config":{
                "spatial_interval":2, "retention":"rolling", "save":"on_trigger", "images":false
            }}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(
        applied["config"], original["config"],
        "active settings must be immutable"
    );
    assert_eq!(applied["next_config"]["spatial_interval"], 2);
    let before = applied["next_config"].clone();
    for patch in [
        json!({"spatial_interval":0}),
        json!({"image_size":9000}),
        json!({"spatial_interval":u64::MAX}),
        json!({"unexpected":true}),
    ] {
        assert!(
            f.query("capture_control", json!({"action":"config","config":patch}))
                .unwrap()
                .is_err()
        );
        assert_eq!(control(&mut f, "status")["next_config"], before);
    }
    control(&mut f, "stop");
    control(&mut f, "discard");
    control(&mut f, "start");
    let marker = f
        .query(
            "recording_marker",
            json!({"source":"human","label":"post-window coverage"}),
        )
        .unwrap()
        .unwrap_data();
    let trigger_frame = marker["frame"].as_u64().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(9);
    let clip = loop {
        let status = control(&mut f, "status");
        if status["last_saved_clip"].is_object() {
            break status["last_saved_clip"].clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "post-window did not finish: {status}"
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    let end = clip["frame_range"][1].as_u64().unwrap();
    assert!(
        end > trigger_frame,
        "post-window must retain later evidence"
    );
    assert_eq!(clip["capture"]["stop_reason"], "marker_post_window");
    assert_eq!(clip["capture"]["post_window_shortened"], false);
    assert_eq!(
        control(&mut f, "status")["state"],
        "recording",
        "explicit rolling trigger resumes"
    );
    f.query("recording_delete", json!({"clip_id":clip["clip_id"]}))
        .unwrap()
        .unwrap_data();
}
