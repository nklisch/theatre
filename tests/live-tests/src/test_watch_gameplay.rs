use crate::stateful_test;
use crate::harness::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

async fn watch_detects_patrol_movement(b: &impl LiveBackend) {
    // Baseline snapshot
    b.stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("baseline snapshot")
        .unwrap_data();

    // Add watch on patrol position
    let watch = b
        .stage(
            "spatial_watch",
            json!({
                "action": "add",
                "watch": { "node": "Enemies/Patrol", "track": ["position"] }
            }),
        )
        .await
        .expect("watch add")
        .unwrap_data();
    let watch_id = watch["watch_id"].as_str().expect("watch_id").to_string();

    // Wait for patrol to move
    b.wait_frames(120).await;

    // Delta should show patrol moved
    let delta = b
        .stage("spatial_delta", json!({}))
        .await
        .expect("delta")
        .unwrap_data();
    let empty = vec![];
    let moved = delta["moved"].as_array().unwrap_or(&empty);
    let patrol_moved = moved.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Patrol"))
            .unwrap_or(false)
    });
    assert!(patrol_moved, "Patrol should appear in delta.moved. Delta: {delta}");

    // Cleanup
    b.stage("spatial_watch", json!({"action": "remove", "watch_id": watch_id}))
        .await
        .expect("watch remove");
}

async fn watch_triggers_on_damage(b: &impl LiveBackend) {
    // Baseline
    b.stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("baseline")
        .unwrap_data();

    // Call take_damage on Stationary enemy
    b.stage(
        "spatial_action",
        json!({
            "action": "call_method",
            "node": "Enemies/Stationary",
            "method": "take_damage",
            "args": [25]
        }),
    )
    .await
    .expect("call take_damage")
    .unwrap_data();

    b.wait_frames(5).await;

    // Delta should show state change
    let delta = b
        .stage("spatial_delta", json!({}))
        .await
        .expect("delta")
        .unwrap_data();
    let empty = vec![];
    let state_changed = delta["state_changed"].as_array().unwrap_or(&empty);
    let stationary_changed = state_changed.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Stationary"))
            .unwrap_or(false)
    });
    assert!(
        stationary_changed,
        "Stationary should appear in delta.state_changed after damage. Delta: {delta}"
    );

    // Verify health via inspect
    let inspect = b
        .stage("spatial_inspect", json!({"node": "Enemies/Stationary"}))
        .await
        .expect("inspect")
        .unwrap_data();
    if let Some(props) = inspect["properties"].as_object() {
        if let Some(health) = props.get("health") {
            let h = health.as_f64().unwrap_or(60.0);
            assert!(
                (h - 35.0).abs() < 0.1,
                "Stationary health should be 35 after 25 damage, got {h}"
            );
        }
    }
}

async fn delta_captures_concurrent_changes(b: &impl LiveBackend) {
    // Baseline
    b.stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("baseline")
        .unwrap_data();

    // Damage stationary enemy
    b.stage(
        "spatial_action",
        json!({
            "action": "call_method",
            "node": "Enemies/Stationary",
            "method": "take_damage",
            "args": [10]
        }),
    )
    .await
    .expect("damage")
    .unwrap_data();

    // Wait for patrol to move + damage to apply
    b.wait_frames(90).await;

    // Delta should show both changes
    let delta = b
        .stage("spatial_delta", json!({}))
        .await
        .expect("delta")
        .unwrap_data();

    let empty = vec![];
    let moved = delta["moved"].as_array().unwrap_or(&empty);
    let state_changed = delta["state_changed"].as_array().unwrap_or(&empty);

    let patrol_moved = moved.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Patrol"))
            .unwrap_or(false)
    });
    let stationary_changed = state_changed.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Stationary"))
            .unwrap_or(false)
    });

    assert!(patrol_moved, "Patrol should be in delta.moved. Delta: {delta}");
    assert!(
        stationary_changed,
        "Stationary should be in delta.state_changed. Delta: {delta}"
    );
}

async fn config_state_properties_tracked_in_delta(b: &impl LiveBackend) {
    // Configure state tracking
    b.stage("spatial_config", json!({"state_properties": ["health"]}))
        .await
        .expect("config")
        .unwrap_data();

    // Baseline
    b.stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("baseline")
        .unwrap_data();

    // Modify health
    b.stage(
        "spatial_action",
        json!({
            "action": "set_property",
            "node": "Enemies/Stationary",
            "property": "health",
            "value": 10
        }),
    )
    .await
    .expect("set_property")
    .unwrap_data();

    b.wait_frames(5).await;

    // Delta should detect health change
    let delta = b
        .stage("spatial_delta", json!({}))
        .await
        .expect("delta")
        .unwrap_data();
    let empty = vec![];
    let state_changed = delta["state_changed"].as_array().unwrap_or(&empty);
    let found = state_changed.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Stationary"))
            .unwrap_or(false)
    });
    assert!(
        found,
        "Stationary should appear in state_changed. Delta: {delta}"
    );
}

stateful_test!(watch_detects_patrol_movement, LIVE_3D, watch_detects_patrol_movement);
stateful_test!(watch_triggers_on_damage, LIVE_3D, watch_triggers_on_damage);
stateful_test!(delta_captures_concurrent_changes, LIVE_3D, delta_captures_concurrent_changes);
stateful_test!(
    config_state_properties_tracked_in_delta,
    LIVE_3D,
    config_state_properties_tracked_in_delta
);
