use crate::dual_test;
use crate::harness::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

async fn screenshot_returns_real_image(b: &impl LiveBackend) {
    // Step 1: wait for buffer to accumulate
    b.wait_frames(120).await;

    // Step 2: save clip
    let save = b
        .stage("clips", json!({"action": "save", "marker_label": "screenshot_test"}))
        .await
        .expect("clips save failed")
        .unwrap_data();
    let clip_id = save["clip_id"].as_str().expect("save should return clip_id").to_string();
    let frames = save["frames"].as_u64().unwrap_or(0);
    assert!(frames > 0, "Clip should contain frames");

    // Step 3: check screenshots list
    let screenshots = b
        .stage("clips", json!({"action": "screenshots", "clip_id": clip_id}))
        .await
        .expect("clips screenshots failed")
        .unwrap_data();
    let total = screenshots["total"].as_u64().expect("total should be present");
    // In windowed mode with GPU, we expect actual screenshots
    assert!(
        total > 0,
        "Windowed Godot should capture screenshots, got total=0"
    );

    // Step 4: cleanup
    b.stage("clips", json!({"action": "delete", "clip_id": clip_id}))
        .await
        .expect("clips delete failed");
}

async fn screenshot_buffer_grows(b: &impl LiveBackend) {
    let status1 = b
        .stage("clips", json!({"action": "status"}))
        .await
        .expect("clips status failed")
        .unwrap_data();
    let count1 = status1["screenshot_buffer_count"].as_u64().unwrap_or(0);

    b.wait_frames(120).await;

    let status2 = b
        .stage("clips", json!({"action": "status"}))
        .await
        .expect("clips status failed")
        .unwrap_data();
    let count2 = status2["screenshot_buffer_count"].as_u64().unwrap_or(0);

    assert!(
        count2 > count1,
        "Screenshot buffer should grow over time: before={count1}, after={count2}"
    );
}

async fn different_timepoints_capture_different_data(b: &impl LiveBackend) {
    // First clip
    b.wait_frames(60).await;
    let save_a = b
        .stage("clips", json!({"action": "save", "marker_label": "clip_a"}))
        .await
        .expect("save a")
        .unwrap_data();
    let clip_a = save_a["clip_id"].as_str().expect("clip_id a").to_string();
    let frames_a = save_a["frames"].as_u64().unwrap_or(0);

    // Second clip (more time)
    b.wait_frames(120).await;
    let save_b = b
        .stage("clips", json!({"action": "save", "marker_label": "clip_b"}))
        .await
        .expect("save b")
        .unwrap_data();
    let clip_b = save_b["clip_id"].as_str().expect("clip_id b").to_string();
    let frames_b = save_b["frames"].as_u64().unwrap_or(0);

    assert!(
        frames_b > frames_a,
        "Second clip should have more frames: a={frames_a}, b={frames_b}"
    );

    // Cleanup
    b.stage("clips", json!({"action": "delete", "clip_id": clip_a}))
        .await
        .ok();
    b.stage("clips", json!({"action": "delete", "clip_id": clip_b}))
        .await
        .ok();
}

dual_test!(screenshot_returns_real_image, LIVE_3D, screenshot_returns_real_image);
dual_test!(screenshot_buffer_grows, LIVE_3D, screenshot_buffer_grows);
dual_test!(
    different_timepoints_capture_different_data,
    LIVE_3D,
    different_timepoints_capture_different_data
);
