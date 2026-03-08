/// Wire tests for recording lifecycle queries.
use crate::harness::GodotFixture;

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn recording_start_status_stop_lifecycle() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let start = f
        .query(
            "recording_start",
            serde_json::json!({
                "name": "wire_test",
                "storage_path": "/tmp/spectator-wire-test/",
                "capture_interval": 1,
                "max_frames": 100
            }),
        )
        .unwrap()
        .unwrap_data();

    assert!(
        !start["recording_id"].as_str().unwrap_or("").is_empty(),
        "recording_id should be non-empty"
    );

    let status = f
        .query("recording_status", serde_json::json!({}))
        .unwrap()
        .unwrap_data();
    assert_eq!(status["active"], true, "recording should be active");

    let stop = f
        .query("recording_stop", serde_json::json!({}))
        .unwrap()
        .unwrap_data();
    assert!(
        stop["frames_captured"].as_u64().is_some(),
        "frames_captured should be present"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn recording_status_when_not_recording() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    let status = f
        .query("recording_status", serde_json::json!({}))
        .unwrap()
        .unwrap_data();

    assert_eq!(status["active"], false, "should not be recording initially");
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn recording_stop_when_not_recording_returns_error_or_ok() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    // Stopping when not recording should either succeed (noop) or return a clear error
    let result = f.query("recording_stop", serde_json::json!({})).unwrap();
    // Both outcomes are acceptable — just shouldn't panic/hang
    let _ = result;
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn recording_add_marker_while_recording() {
    let mut f = GodotFixture::start("test_scene_3d.tscn").unwrap();

    f.query(
        "recording_start",
        serde_json::json!({
            "name": "marker_test",
            "storage_path": "/tmp/spectator-wire-test/",
            "capture_interval": 1,
            "max_frames": 100
        }),
    )
    .unwrap()
    .unwrap_data();

    let result = f
        .query(
            "recording_add_marker",
            serde_json::json!({
                "source": "wire_test",
                "label": "test_marker"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(result["result"], "ok");

    f.query("recording_stop", serde_json::json!({}))
        .unwrap()
        .unwrap_data();
}
