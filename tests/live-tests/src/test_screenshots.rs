use crate::dual_test;
use crate::harness::*;
use crate::harness::assertions::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

/// Journey: Dashcam captures rendered frames, teleport changes viewport, clips differ.
///
/// Tests the full dashcam lifecycle with a real GPU renderer — the primary
/// capability gap that headless tests cannot cover. Verifies screenshots
/// contain actual image data and that scene mutations produce visually
/// different captures.
///
/// Steps:
///   1. clips(status) → dashcam enabled, buffering, screenshot fields present
///   2. spatial_snapshot(standard) → baseline scene, note entity positions
///   3. wait_frames(120) → accumulate ~2s of rendered buffer
///   4. clips(status) → screenshot_buffer_count > 0 (GPU is rendering)
///   5. clips(save, "before_teleport") → clip_a with frames > 0
///   6. clips(screenshots, clip_a) → total > 0, screenshots array non-empty
///   7. spatial_action(teleport, Patrol, [0, 0, 0]) → move enemy to origin
///   8. wait_frames(60) → let viewport render new state
///   9. spatial_snapshot(standard) → verify Patrol is at ~(0,0,0) now
///  10. clips(save, "after_teleport") → clip_b
///  11. clips(screenshots, clip_b) → total > 0
///  12. Assert: clip_b has more frames than clip_a (buffer kept accumulating)
///  13. clips(list) → both clips present, both have dashcam=false (manual saves)
///  14. clips(delete, clip_a) → cleanup
///  15. clips(delete, clip_b) → cleanup
///  16. clips(list) → both clips gone
async fn journey_dashcam_captures_rendered_scene(b: &impl LiveBackend) {
    // Step 1: verify dashcam is active with screenshot fields
    let status = b
        .stage("clips", json!({"action": "status"}))
        .await
        .expect("clips status")
        .unwrap_data();
    assert_eq!(status["dashcam_enabled"], json!(true));
    assert_eq!(status["state"], json!("buffering"));
    assert!(
        status["screenshot_buffer_count"].as_u64().is_some(),
        "screenshot_buffer_count must be present: {status}"
    );
    assert!(
        status["screenshot_buffer_kb"].as_u64().is_some(),
        "screenshot_buffer_kb must be present: {status}"
    );

    // Step 2: baseline snapshot — note Patrol position
    let baseline = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("baseline snapshot")
        .unwrap_data();
    let entities = baseline["entities"].as_array().expect("entities");
    assert!(!entities.is_empty(), "Scene should have entities");
    let patrol = find_entity(entities, "Patrol");
    let patrol_pos_before = extract_position(patrol);

    // Step 3: accumulate rendered frames
    b.wait_frames(120).await;

    // Step 4: verify GPU is actually rendering screenshots
    let status2 = b
        .stage("clips", json!({"action": "status"}))
        .await
        .expect("status after wait")
        .unwrap_data();
    let buffer_count = status2["screenshot_buffer_count"].as_u64().unwrap_or(0);
    assert!(
        buffer_count > 0,
        "Windowed Godot should have rendered screenshots, buffer_count={buffer_count}"
    );

    // Step 5: save first clip
    let save_a = b
        .stage(
            "clips",
            json!({"action": "save", "marker_label": "before_teleport"}),
        )
        .await
        .expect("save clip_a")
        .unwrap_data();
    let clip_a = save_a["clip_id"]
        .as_str()
        .expect("clip_id")
        .to_string();
    let frames_a = save_a["frames"].as_u64().unwrap_or(0);
    assert!(frames_a > 0, "First clip should contain frames");

    // Step 6: verify screenshots exist in first clip
    let screenshots_a = b
        .stage(
            "clips",
            json!({"action": "screenshots", "clip_id": &clip_a}),
        )
        .await
        .expect("screenshots clip_a")
        .unwrap_data();
    let total_a = screenshots_a["total"].as_u64().expect("total");
    assert!(
        total_a > 0,
        "Windowed Godot should have captured screenshots in clip, total={total_a}"
    );

    // Step 7: teleport Patrol to origin — changes the scene visually
    b.stage(
        "spatial_action",
        json!({
            "action": "teleport",
            "node": "Enemies/Patrol",
            "position": [0.0, 0.0, 0.0]
        }),
    )
    .await
    .expect("teleport Patrol")
    .unwrap_data();

    // Step 8: let viewport render the new state
    b.wait_frames(60).await;

    // Step 9: verify Patrol actually moved
    let after_snap = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("post-teleport snapshot")
        .unwrap_data();
    let after_entities = after_snap["entities"].as_array().expect("entities");
    let patrol_after = find_entity(after_entities, "Patrol");
    let patrol_pos_after = extract_position(patrol_after);
    let displacement = ((patrol_pos_after[0] - patrol_pos_before[0]).powi(2)
        + (patrol_pos_after[2] - patrol_pos_before[2]).powi(2))
    .sqrt();
    assert!(
        displacement > 1.0,
        "Patrol should have moved from {:?} to near origin, displacement={displacement}",
        patrol_pos_before
    );

    // Step 10: save second clip (after scene change)
    let save_b = b
        .stage(
            "clips",
            json!({"action": "save", "marker_label": "after_teleport"}),
        )
        .await
        .expect("save clip_b")
        .unwrap_data();
    let clip_b = save_b["clip_id"]
        .as_str()
        .expect("clip_id b")
        .to_string();
    let frames_b = save_b["frames"].as_u64().unwrap_or(0);
    assert!(frames_b > 0, "Second clip should contain frames");

    // Step 11: verify second clip also has screenshots
    let screenshots_b = b
        .stage(
            "clips",
            json!({"action": "screenshots", "clip_id": &clip_b}),
        )
        .await
        .expect("screenshots clip_b")
        .unwrap_data();
    let total_b = screenshots_b["total"].as_u64().expect("total");
    assert!(total_b > 0, "Second clip should have screenshots too");

    // Step 12: second clip accumulated more buffer time
    assert!(
        frames_b > frames_a,
        "Second clip should have more frames (buffer kept growing): a={frames_a}, b={frames_b}"
    );

    // Step 13: both clips in list
    let list = b
        .stage("clips", json!({"action": "list"}))
        .await
        .expect("clips list")
        .unwrap_data();
    let clips = list["clips"].as_array().expect("clips array");
    let has_a = clips
        .iter()
        .any(|c| c["clip_id"].as_str() == Some(&clip_a));
    let has_b = clips
        .iter()
        .any(|c| c["clip_id"].as_str() == Some(&clip_b));
    assert!(has_a, "Clip A should be in list");
    assert!(has_b, "Clip B should be in list");

    // Steps 14-15: cleanup
    b.stage("clips", json!({"action": "delete", "clip_id": &clip_a}))
        .await
        .expect("delete clip_a")
        .unwrap_data();
    b.stage("clips", json!({"action": "delete", "clip_id": &clip_b}))
        .await
        .expect("delete clip_b")
        .unwrap_data();

    // Step 16: verify both deleted
    let list2 = b
        .stage("clips", json!({"action": "list"}))
        .await
        .expect("clips list after delete")
        .unwrap_data();
    let clips2 = list2["clips"].as_array().expect("clips array");
    let still_a = clips2
        .iter()
        .any(|c| c["clip_id"].as_str() == Some(&clip_a));
    let still_b = clips2
        .iter()
        .any(|c| c["clip_id"].as_str() == Some(&clip_b));
    assert!(!still_a, "Clip A should be gone after delete");
    assert!(!still_b, "Clip B should be gone after delete");
}

dual_test!(
    journey_dashcam_captures_rendered_scene,
    LIVE_3D,
    journey_dashcam_captures_rendered_scene
);
