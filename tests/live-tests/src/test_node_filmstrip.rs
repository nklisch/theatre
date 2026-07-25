use crate::dual_test;
use crate::harness::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

async fn journey_follows_moving_node(b: &impl LiveBackend) {
    b.wait_frames(120).await;
    let save = b
        .stage("clips", json!({"action":"save","marker_label":"filmstrip"}))
        .await
        .expect("save")
        .unwrap_data();
    let clip_id = save["clip_id"].as_str().expect("clip id").to_owned();
    let status = b
        .stage("clips", json!({"action":"status"}))
        .await
        .expect("status")
        .unwrap_data();
    let screenshots = status["screenshots_available"].as_bool().unwrap_or(false)
        || status["screenshot_buffer_count"].as_u64().unwrap_or(0) > 0;
    let artifact = b.stage("clips", json!({"action":"visual_artifact","artifact":"node_filmstrip","clip_id":clip_id,"node":"Enemies/Patrol"})).await.expect("artifact").unwrap_data();
    if screenshots {
        assert_eq!(artifact["kind"], json!("node_filmstrip"));
        assert!(
            artifact["projection"]["counts"]["on_screen"]
                .as_u64()
                .unwrap_or(0)
                > 0
        );
        assert!(artifact["image"].is_object());
        let repeat = b.stage("clips", json!({"action":"visual_artifact","artifact":"node_filmstrip","clip_id":clip_id,"node":"Enemies/Patrol"})).await.expect("cached artifact").unwrap_data();
        assert_eq!(repeat["image"]["cache"], json!("hit"));
    } else {
        assert_eq!(artifact["error"], json!("no_screenshots"));
    }
    b.stage("clips", json!({"action":"delete","clip_id":clip_id}))
        .await
        .expect("cleanup")
        .unwrap_data();
}

dual_test!(
    journey_follows_moving_node,
    LIVE_3D,
    journey_follows_moving_node
);

async fn journey_unknown_node_reports_paths(b: &impl LiveBackend) {
    b.wait_frames(60).await;
    let save = b
        .stage(
            "clips",
            json!({"action":"save","marker_label":"unknown node"}),
        )
        .await
        .expect("save")
        .unwrap_data();
    let clip_id = save["clip_id"].as_str().expect("clip id").to_owned();
    let result = b.stage("clips", json!({"action":"visual_artifact","artifact":"node_filmstrip","clip_id":clip_id,"node":"Enemies/DefinitelyMissing"})).await.expect("artifact response").unwrap_data();
    assert_eq!(result["error"], json!("node_not_found"));
    assert!(
        !result["sample_paths"]
            .as_array()
            .expect("sample_paths")
            .is_empty()
    );
    b.stage("clips", json!({"action":"delete","clip_id":clip_id}))
        .await
        .expect("cleanup")
        .unwrap_data();
}

dual_test!(
    journey_unknown_node_reports_paths,
    LIVE_3D,
    journey_unknown_node_reports_paths
);
