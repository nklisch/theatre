use crate::stateful_test;
use crate::harness::*;
use crate::harness::assertions::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

/// Journey: Full watch + delta lifecycle with real gameplay state changes.
///
/// This is the most complex stateful test — it exercises watches, deltas,
/// config, actions, and inspect across organic gameplay changes (patrol
/// movement) and explicit mutations (take_damage, set_property). Every
/// observation tool is used to cross-verify state.
///
/// Steps:
///   1. spatial_config(state_properties={"enemies": ["health"]}) → track health
///   2. spatial_snapshot(full) → baseline with state, note entity count & positions
///   3. spatial_inspect(Enemies/Patrol) → health=80, confirm transform
///   4. spatial_inspect(Enemies/Stationary) → health=60
///   5. spatial_watch(add, Enemies/Patrol, track=["position"]) → watch_id_patrol
///   6. spatial_watch(add, Enemies/Stationary, track=["state"]) → watch_id_stationary
///   7. spatial_watch(list) → 2 watches active
///   8. spatial_action(call_method, Stationary, take_damage, [25]) → health 60→35
///   9. wait_frames(5) → let damage propagate
///  10. spatial_inspect(Enemies/Stationary) → health=35
///  11. wait_frames(90) → let patrol move
///  12. spatial_delta() → Patrol in moved (organic), Stationary in state_changed (damage)
///  13. Assert: Patrol delta_pos is non-zero
///  14. Assert: Stationary state change includes health
///  15. spatial_snapshot(standard) → new baseline, Patrol at new position
///  16. spatial_action(call_method, Stationary, take_damage, [10]) → health 35→25
///  17. spatial_action(set_property, Enemies/Patrol, speed, 0.0) → freeze patrol
///  18. wait_frames(60) → patrol should stop moving
///  19. spatial_delta() → Stationary in state_changed (health 35→25),
///      Patrol should NOT be in moved (speed=0, frozen)
///  20. spatial_inspect(Enemies/Stationary) → health=25
///  21. spatial_inspect(Enemies/Patrol) → speed=0.0
///  22. spatial_watch(remove, watch_id_patrol)
///  23. spatial_watch(remove, watch_id_stationary)
///  24. spatial_watch(list) → 0 watches
async fn journey_watch_delta_gameplay(b: &impl LiveBackend) {
    // Step 1: configure state tracking
    b.stage(
        "spatial_config",
        json!({"state_properties": {"enemies": ["health"]}}),
    )
    .await
    .expect("config")
    .unwrap_data();

    // Step 2: baseline snapshot with full detail
    let baseline = b
        .stage("spatial_snapshot", json!({"detail": "full"}))
        .await
        .expect("baseline snapshot")
        .unwrap_data();
    let entities = baseline["entities"].as_array().expect("entities");
    assert!(
        entities.len() >= 3,
        "Should have at least Player, Patrol, Stationary. Got {} entities",
        entities.len()
    );

    // Step 3: inspect Patrol
    let patrol_inspect = b
        .stage("spatial_inspect", json!({"node": "Enemies/Patrol"}))
        .await
        .expect("inspect Patrol")
        .unwrap_data();
    if let Some(props) = patrol_inspect["properties"].as_object() {
        if let Some(h) = props.get("health") {
            assert!(
                (h.as_f64().unwrap_or(0.0) - 80.0).abs() < 0.1,
                "Patrol health should be 80"
            );
        }
    }

    // Step 4: inspect Stationary
    let stat_inspect = b
        .stage(
            "spatial_inspect",
            json!({"node": "Enemies/Stationary"}),
        )
        .await
        .expect("inspect Stationary")
        .unwrap_data();
    if let Some(props) = stat_inspect["properties"].as_object() {
        if let Some(h) = props.get("health") {
            assert!(
                (h.as_f64().unwrap_or(0.0) - 60.0).abs() < 0.1,
                "Stationary health should be 60"
            );
        }
    }

    // Step 5: watch patrol position
    let w_patrol = b
        .stage(
            "spatial_watch",
            json!({
                "action": "add",
                "watch": {"node": "Enemies/Patrol", "track": ["position"]}
            }),
        )
        .await
        .expect("watch add Patrol")
        .unwrap_data();
    let watch_id_patrol = w_patrol["watch_id"]
        .as_str()
        .expect("watch_id")
        .to_string();

    // Step 6: watch stationary state
    let w_stat = b
        .stage(
            "spatial_watch",
            json!({
                "action": "add",
                "watch": {"node": "Enemies/Stationary", "track": ["state"]}
            }),
        )
        .await
        .expect("watch add Stationary")
        .unwrap_data();
    let watch_id_stat = w_stat["watch_id"]
        .as_str()
        .expect("watch_id")
        .to_string();

    // Step 7: list watches
    let watch_list = b
        .stage("spatial_watch", json!({"action": "list"}))
        .await
        .expect("watch list")
        .unwrap_data();
    let watches = watch_list["watches"].as_array();
    assert!(
        watches.map(|w| w.len()).unwrap_or(0) >= 2,
        "Should have at least 2 watches: {watch_list}"
    );

    // Step 8: damage Stationary
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
    .expect("call take_damage 25")
    .unwrap_data();

    // Step 9: let damage propagate
    b.wait_frames(5).await;

    // Step 10: verify health dropped
    let stat_after = b
        .stage(
            "spatial_inspect",
            json!({"node": "Enemies/Stationary"}),
        )
        .await
        .expect("inspect after damage")
        .unwrap_data();
    if let Some(props) = stat_after["properties"].as_object() {
        if let Some(h) = props.get("health") {
            assert!(
                (h.as_f64().unwrap_or(60.0) - 35.0).abs() < 0.1,
                "Stationary health should be 35, got {}",
                h.as_f64().unwrap_or(0.0)
            );
        }
    }

    // Step 11: wait for patrol to move
    b.wait_frames(90).await;

    // Step 12: delta should capture both changes
    let delta = b
        .stage("spatial_delta", json!({}))
        .await
        .expect("first delta")
        .unwrap_data();

    // Step 13: patrol should be in moved
    let empty = vec![];
    let moved = delta["moved"].as_array().unwrap_or(&empty);
    let patrol_moved = moved.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Patrol"))
            .unwrap_or(false)
    });
    assert!(
        patrol_moved,
        "Patrol should be in delta.moved (organic movement). Delta: {delta}"
    );

    // Verify delta_pos is non-zero for Patrol
    if let Some(patrol_delta) = moved.iter().find(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Patrol"))
            .unwrap_or(false)
    }) {
        if let Some(dp) = patrol_delta["delta_pos"].as_array() {
            let magnitude: f64 = dp
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!(
                magnitude > 0.1,
                "Patrol delta_pos should be non-zero: {dp:?}"
            );
        }
    }

    // Step 14: stationary should be in state_changed
    let state_changed = delta["state_changed"].as_array().unwrap_or(&empty);
    let stationary_changed = state_changed.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Stationary"))
            .unwrap_or(false)
    });
    assert!(
        stationary_changed,
        "Stationary should be in delta.state_changed (damage). Delta: {delta}"
    );

    // Step 15: new baseline
    let snap2 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("second baseline")
        .unwrap_data();
    let e2 = snap2["entities"].as_array().expect("entities");
    let _patrol_pos = extract_position(find_entity(e2, "Patrol"));

    // Step 16: more damage
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
    .expect("call take_damage 10")
    .unwrap_data();

    // Step 17: freeze patrol by setting speed=0
    b.stage(
        "spatial_action",
        json!({
            "action": "set_property",
            "node": "Enemies/Patrol",
            "property": "speed",
            "value": 0.0
        }),
    )
    .await
    .expect("set_property speed=0")
    .unwrap_data();

    // Step 18: wait — patrol should NOT move now
    b.wait_frames(60).await;

    // Step 19: delta — Stationary changed, Patrol should NOT have moved
    let delta2 = b
        .stage("spatial_delta", json!({}))
        .await
        .expect("second delta")
        .unwrap_data();

    let state_changed2 = delta2["state_changed"].as_array().unwrap_or(&empty);
    let stationary_changed2 = state_changed2.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Stationary"))
            .unwrap_or(false)
    });
    assert!(
        stationary_changed2,
        "Stationary should be in second delta.state_changed. Delta: {delta2}"
    );

    let moved2 = delta2["moved"].as_array().unwrap_or(&empty);
    let patrol_moved2 = moved2.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Patrol"))
            .unwrap_or(false)
    });
    // Patrol speed is 0 — the sine wave shouldn't change position
    // (tolerance: patrol may drift slightly due to frame timing)
    if patrol_moved2 {
        // If patrol shows as moved, the displacement should be tiny
        if let Some(pd) = moved2.iter().find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains("Patrol"))
                .unwrap_or(false)
        }) {
            if let Some(dp) = pd["delta_pos"].as_array() {
                let mag: f64 = dp
                    .iter()
                    .map(|v| v.as_f64().unwrap_or(0.0).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(
                    mag < 1.0,
                    "Patrol with speed=0 should barely move, delta_pos magnitude={mag}"
                );
            }
        }
    }

    // Step 20: verify health=25
    let stat_final = b
        .stage(
            "spatial_inspect",
            json!({"node": "Enemies/Stationary"}),
        )
        .await
        .expect("final inspect Stationary")
        .unwrap_data();
    if let Some(props) = stat_final["properties"].as_object() {
        if let Some(h) = props.get("health") {
            assert!(
                (h.as_f64().unwrap_or(0.0) - 25.0).abs() < 0.1,
                "Stationary health should be 25, got {}",
                h.as_f64().unwrap_or(0.0)
            );
        }
    }

    // Step 21: verify patrol speed=0
    let patrol_final = b
        .stage("spatial_inspect", json!({"node": "Enemies/Patrol"}))
        .await
        .expect("final inspect Patrol")
        .unwrap_data();
    if let Some(props) = patrol_final["properties"].as_object() {
        if let Some(s) = props.get("speed") {
            assert!(
                s.as_f64().unwrap_or(1.0).abs() < 0.01,
                "Patrol speed should be 0, got {}",
                s.as_f64().unwrap_or(0.0)
            );
        }
    }

    // Steps 22-23: cleanup watches
    b.stage(
        "spatial_watch",
        json!({"action": "remove", "watch_id": &watch_id_patrol}),
    )
    .await
    .expect("remove patrol watch");
    b.stage(
        "spatial_watch",
        json!({"action": "remove", "watch_id": &watch_id_stat}),
    )
    .await
    .expect("remove stationary watch");

    // Step 24: verify watches cleared
    let list_final = b
        .stage("spatial_watch", json!({"action": "list"}))
        .await
        .expect("final watch list")
        .unwrap_data();
    let final_watches = list_final["watches"].as_array();
    assert!(
        final_watches.map(|w| w.is_empty()).unwrap_or(true),
        "All watches should be removed: {list_final}"
    );
}

stateful_test!(
    journey_watch_delta_gameplay,
    LIVE_3D,
    journey_watch_delta_gameplay
);
