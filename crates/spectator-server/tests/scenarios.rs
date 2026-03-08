//! Multi-step integration tests: cross-tool state dependencies and ordering invariants.
//!
//! These tests focus on scenarios where bugs hide — sequences of tool calls that share
//! state, ordering constraints, and interactions between tools that appear independent.
//!
//! Run with: cargo test -p spectator-server --features integration-tests

mod support;

use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use tokio::time::{Duration, sleep};

use support::{
    fixtures::{entity, mock_scene_3d},
    harness::TestHarness,
    mock_addon::QueryHandler,
};

// ============================================================================
// Helpers
// ============================================================================

/// Returns a handler that serves a fixed scene for snapshot, and a minimal
/// raycast response for spatial_query.  All other methods error.
fn scene_handler(scene: serde_json::Value) -> QueryHandler {
    let scene = Arc::new(scene);
    Arc::new(move |method: &str, _params: &serde_json::Value| match method {
        "get_snapshot_data" => Ok((*scene).clone()),
        "spatial_query" => Ok(json!({
            "clear": true,
            "total_distance": 10.0,
            "clear_distance": 10.0
        })),
        _ => Err(("unknown".into(), format!("unexpected method: {method}"))),
    })
}

/// Start a harness with a fixed scene and build the spatial index immediately.
async fn harness_with_snapshot(scene: serde_json::Value) -> TestHarness {
    let harness = TestHarness::new(scene_handler(scene)).await;
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();
    harness
}

// ============================================================================
// Section 1: Spatial index coherence
// snapshot builds the index; spatial_query reads it — they must agree
// ============================================================================

/// Radius query must only return entities within the requested distance.
///
/// Scene geometry (all at y=0):
///   Player  [0,0,0]    dist from origin: 0
///   Coin    [0,0,2]    dist: 2
///   EastWall[3,0,0]    dist: 3
///   Scout   [0,0,-5]   dist: 5
///   Camera  [0,5,10]   dist: ~11
///
/// radius=2.5 → Player + Coin; EastWall and Scout excluded.
#[tokio::test]
async fn test_radius_query_respects_entity_positions() {
    let scene = serde_json::to_value(mock_scene_3d()).unwrap();
    let harness = harness_with_snapshot(scene).await;

    let result = harness
        .call_tool(
            "spatial_query",
            json!({
                "query_type": "radius",
                "from": [0.0, 0.0, 0.0],
                "radius": 2.5
            }),
        )
        .await
        .unwrap();

    let results = result["results"]
        .as_array()
        .expect("spatial_query should return results array");
    let paths: Vec<&str> = results
        .iter()
        .filter_map(|e| e["path"].as_str())
        .collect();

    assert!(
        paths.contains(&"Player"),
        "Player at origin should be within 2.5m: {result}"
    );
    assert!(
        paths.contains(&"items/Coin"),
        "Coin at (0,0,2) should be within 2.5m: {result}"
    );
    assert!(
        !paths.contains(&"walls/EastWall"),
        "EastWall at (3,0,0) should be outside 2.5m: {result}"
    );
    assert!(
        !paths.contains(&"enemies/Scout"),
        "Scout at (0,0,-5) should be outside 2.5m: {result}"
    );
}

/// nearest(k=1) from a point returns the closest entity, not just any entity.
#[tokio::test]
async fn test_nearest_returns_geometrically_closest() {
    let scene = serde_json::to_value(mock_scene_3d()).unwrap();
    let harness = harness_with_snapshot(scene).await;

    // Query from EastWall's position — EastWall should be closest (distance ≈ 0)
    let result = harness
        .call_tool(
            "spatial_query",
            json!({
                "query_type": "nearest",
                "from": [3.0, 0.0, 0.0],
                "k": 1
            }),
        )
        .await
        .unwrap();

    let results = result["results"]
        .as_array()
        .expect("nearest should return results");
    assert_eq!(results.len(), 1, "k=1 should return exactly 1 result");
    assert_eq!(
        results[0]["path"], "walls/EastWall",
        "nearest to (3,0,0) should be EastWall: {result}"
    );
}

/// spatial_query uses the index from the LAST snapshot, not live scene data.
/// After refreshing the snapshot, queries should see the new scene.
#[tokio::test]
async fn test_query_index_updates_on_snapshot_refresh() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let handler: QueryHandler = Arc::new(move |method, _| {
        if method != "get_snapshot_data" {
            return Err(("unknown".into(), format!("unexpected: {method}")));
        }
        let n = cc.fetch_add(1, Ordering::SeqCst);
        let mut scene = mock_scene_3d();
        if n >= 1 {
            // Second+ snapshot: add a new enemy close to origin
            scene.entities.push(entity(
                "enemies/Ambusher",
                "CharacterBody3D",
                [0.5, 0.0, 0.0],
                &["enemies"],
            ));
        }
        Ok(serde_json::to_value(scene).unwrap())
    });

    let harness = TestHarness::new(handler).await;

    // First snapshot — Ambusher not in scene
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Query: Ambusher should NOT be in the index yet
    let q1 = harness
        .call_tool(
            "spatial_query",
            json!({ "query_type": "radius", "from": [0.0,0.0,0.0], "radius": 5.0 }),
        )
        .await
        .unwrap();
    let empty = vec![];
    let paths1: Vec<&str> = q1["results"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|e| e["path"].as_str())
        .collect();
    assert!(
        !paths1.contains(&"enemies/Ambusher"),
        "Ambusher should not appear before snapshot refresh: {q1}"
    );

    // Refresh snapshot — Ambusher now added to scene
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Query again: Ambusher should now appear
    let q2 = harness
        .call_tool(
            "spatial_query",
            json!({ "query_type": "radius", "from": [0.0,0.0,0.0], "radius": 5.0 }),
        )
        .await
        .unwrap();
    let empty2 = vec![];
    let paths2: Vec<&str> = q2["results"]
        .as_array()
        .unwrap_or(&empty2)
        .iter()
        .filter_map(|e| e["path"].as_str())
        .collect();
    assert!(
        paths2.contains(&"enemies/Ambusher"),
        "Ambusher should appear after snapshot refresh: {q2}"
    );
}

// ============================================================================
// Section 2: Delta baseline semantics
// Each delta compares against the most recent state, not the original snapshot
// ============================================================================

/// The delta baseline rolls forward: after two snapshots A→B, a delta comparing
/// against B (not A) should report no movement if the scene hasn't changed since B.
#[tokio::test]
async fn test_delta_baseline_rolls_forward_on_snapshot() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let handler: QueryHandler = Arc::new(move |method, _| {
        if method != "get_snapshot_data" {
            return Err(("unknown".into(), format!("unexpected: {method}")));
        }
        let n = cc.fetch_add(1, Ordering::SeqCst);
        let mut scene = mock_scene_3d();
        scene.frame = 100 + n as u64 * 10;
        if n >= 1 {
            // All calls after the first: Scout has already moved
            scene.entities[1].position = vec![10.0, 0.0, 0.0];
        }
        Ok(serde_json::to_value(scene).unwrap())
    });

    let harness = TestHarness::new(handler).await;

    // Snapshot 1: Scout at [0,0,-5] — this is the initial baseline
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Snapshot 2: Scout at [10,0,0] — replaces the baseline
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Delta: current state (Scout still at [10,0,0]) vs baseline (also [10,0,0])
    // Scout did NOT move since the last snapshot → should NOT appear in moved
    let delta = harness
        .call_tool("spatial_delta", json!({}))
        .await
        .unwrap();

    let no_moved = vec![];
    let moved = delta["moved"].as_array().unwrap_or(&no_moved);
    assert!(
        !moved.iter().any(|e| e["path"] == "enemies/Scout"),
        "Scout at same position as last snapshot should NOT be in moved: {delta}"
    );
}

/// Successive delta calls each compare against the state from the previous delta,
/// so an entity that moves between delta1 and delta2 appears in delta2 but not delta3
/// if it stops moving.
#[tokio::test]
async fn test_delta_successive_calls_track_incremental_movement() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    // Snapshot call sequence: call 0 = baseline, call 1 = moved, call 2 = moved again, call 3 = stopped
    let handler: QueryHandler = Arc::new(move |method, _| {
        if method != "get_snapshot_data" {
            return Err(("unknown".into(), format!("unexpected: {method}")));
        }
        let n = cc.fetch_add(1, Ordering::SeqCst);
        let mut scene = mock_scene_3d();
        scene.frame = 100 + n as u64;
        scene.entities[1].position = match n {
            0 => vec![0.0, 0.0, -5.0],  // baseline: Scout at origin
            1 => vec![5.0, 0.0, 0.0],   // delta1: Scout moved
            2 => vec![15.0, 0.0, 0.0],  // delta2: Scout moved again
            _ => vec![15.0, 0.0, 0.0],  // delta3+: Scout stopped
        };
        Ok(serde_json::to_value(scene).unwrap())
    });

    let harness = TestHarness::new(handler).await;

    // Establish baseline (call 0)
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Delta 1 (call 1): Scout moved from [0,0,-5] to [5,0,0]
    let d1 = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    assert!(
        d1["moved"]
            .as_array()
            .map(|a| a.iter().any(|e| e["path"] == "enemies/Scout"))
            .unwrap_or(false),
        "delta 1 should see Scout move to [5,0,0]: {d1}"
    );

    // Delta 2 (call 2): Scout moved from [5,0,0] to [15,0,0]
    let d2 = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    assert!(
        d2["moved"]
            .as_array()
            .map(|a| a.iter().any(|e| e["path"] == "enemies/Scout"))
            .unwrap_or(false),
        "delta 2 should see Scout move to [15,0,0]: {d2}"
    );

    // Delta 3 (call 3+): Scout at [15,0,0] — same as after delta 2 → no movement
    let d3 = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    assert!(
        !d3["moved"]
            .as_array()
            .map(|a| a.iter().any(|e| e["path"] == "enemies/Scout"))
            .unwrap_or(false),
        "delta 3 should NOT see Scout move (it stopped): {d3}"
    );
}

// ============================================================================
// Section 3: Stateful action → snapshot round-trip
// The mock tracks mutable game state so action effects are visible in snapshot
// ============================================================================

/// After teleporting a node, the next snapshot should reflect the new position.
/// Requires a stateful mock that tracks the current player position.
#[tokio::test]
async fn test_teleport_reflected_in_subsequent_snapshot() {
    let player_pos = Arc::new(std::sync::Mutex::new([0.0_f64, 0.0, 0.0]));
    let pos = player_pos.clone();

    let handler: QueryHandler = Arc::new(move |method, params| {
        match method {
            "execute_action" if params["action"].as_str() == Some("teleport") => {
                let arr = params["position"].as_array();
                if let Some(a) = arr {
                    let x = a.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let z = a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    *pos.lock().unwrap() = [x, y, z];
                }
                Ok(json!({ "success": true, "previous_position": [0.0, 0.0, 0.0] }))
            }
            "get_snapshot_data" => {
                let current = *pos.lock().unwrap();
                let mut scene = mock_scene_3d();
                scene.entities[0].position = current.to_vec(); // Player is index 0
                Ok(serde_json::to_value(scene).unwrap())
            }
            _ => Err(("unknown".into(), format!("unexpected: {method}"))),
        }
    });

    let harness = TestHarness::new(handler).await;

    // Teleport Player to [20, 0, 5]
    harness
        .call_tool(
            "spatial_action",
            json!({
                "action": "teleport",
                "node": "Player",
                "position": [20.0, 0.0, 5.0]
            }),
        )
        .await
        .unwrap();

    // Snapshot with wide radius to ensure Player is captured
    let snapshot = harness
        .call_tool(
            "spatial_snapshot",
            json!({ "detail": "standard", "radius": 100.0 }),
        )
        .await
        .unwrap();

    let entities = snapshot["entities"].as_array().unwrap();
    let player = entities
        .iter()
        .find(|e| e["path"] == "Player")
        .expect("Player should appear in snapshot");

    // The absolute position should reflect the teleport destination
    let abs = player["abs"].as_array().expect("Player should have abs position");
    let x = abs[0].as_f64().unwrap_or(f64::NAN);
    assert!(
        (x - 20.0).abs() < 0.5,
        "Player x should be ~20.0 after teleport, got {x}: {player}"
    );
}

/// Teleporting a node also updates the spatial index: queries after a post-teleport
/// snapshot should find the node at its new location, not the old one.
#[tokio::test]
async fn test_teleport_then_snapshot_updates_spatial_index() {
    let enemy_pos = Arc::new(std::sync::Mutex::new([0.0_f64, 0.0, -5.0])); // Scout's initial position
    let pos = enemy_pos.clone();

    let handler: QueryHandler = Arc::new(move |method, params| {
        match method {
            "execute_action" if params["action"].as_str() == Some("teleport") => {
                if let Some(a) = params["position"].as_array() {
                    let x = a.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let z = a.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    *pos.lock().unwrap() = [x, y, z];
                }
                Ok(json!({ "success": true, "previous_position": [0.0, 0.0, -5.0] }))
            }
            "get_snapshot_data" => {
                let current = *pos.lock().unwrap();
                let mut scene = mock_scene_3d();
                scene.entities[1].position = current.to_vec(); // Scout is index 1
                Ok(serde_json::to_value(scene).unwrap())
            }
            _ => Ok(json!({ "clear": true, "total_distance": 10.0, "clear_distance": 10.0 })),
        }
    });

    let harness = TestHarness::new(handler).await;

    // Initial snapshot: Scout at [0,0,-5]
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Scout at [0,0,-5] is NOT within radius 2 of origin
    let q1 = harness
        .call_tool(
            "spatial_query",
            json!({ "query_type": "radius", "from": [0.0,0.0,0.0], "radius": 2.0 }),
        )
        .await
        .unwrap();
    let empty_before = vec![];
    let in_range_before: Vec<&str> = q1["results"]
        .as_array()
        .unwrap_or(&empty_before)
        .iter()
        .filter_map(|e| e["path"].as_str())
        .collect();
    assert!(
        !in_range_before.contains(&"enemies/Scout"),
        "Scout should be outside radius 2 before teleport: {q1}"
    );

    // Teleport Scout to [0,0,0] (right at the origin)
    harness
        .call_tool(
            "spatial_action",
            json!({ "action": "teleport", "node": "enemies/Scout", "position": [0.0, 0.0, 0.0] }),
        )
        .await
        .unwrap();

    // Refresh snapshot to update the spatial index
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Scout should now be within radius 2 of origin
    let q2 = harness
        .call_tool(
            "spatial_query",
            json!({ "query_type": "radius", "from": [0.0,0.0,0.0], "radius": 2.0 }),
        )
        .await
        .unwrap();
    let empty_after = vec![];
    let in_range_after: Vec<&str> = q2["results"]
        .as_array()
        .unwrap_or(&empty_after)
        .iter()
        .filter_map(|e| e["path"].as_str())
        .collect();
    assert!(
        in_range_after.contains(&"enemies/Scout"),
        "Scout should be within radius 2 after teleport to origin: {q2}"
    );
}

// ============================================================================
// Section 4: Config propagation — settings applied to subsequent calls
// ============================================================================

/// token_budget in the request limits the response size; budget block must report usage.
#[tokio::test]
async fn test_token_budget_enforced_in_snapshot() {
    let scene = serde_json::to_value(mock_scene_3d()).unwrap();
    let harness = TestHarness::new(scene_handler(scene)).await;

    // Request with a tiny budget
    let result = harness
        .call_tool(
            "spatial_snapshot",
            json!({ "detail": "standard", "token_budget": 150 }),
        )
        .await
        .unwrap();

    let budget = result.get("budget").expect("budget block must be present");
    let used = budget["used"].as_u64().unwrap_or(0);
    let limit = budget["limit"].as_u64().unwrap_or(u64::MAX);
    // The server may add a small overhead for the budget block itself, but
    // used tokens should not vastly exceed the limit
    assert!(
        used <= limit + 50,
        "budget.used ({used}) should be at or near limit ({limit}): {result}"
    );
}

/// spatial_config sets the session token_hard_cap; subsequent snapshot calls
/// must report that hard cap in their budget block.
#[tokio::test]
async fn test_config_hard_cap_propagates_to_snapshot_budget() {
    let scene = serde_json::to_value(mock_scene_3d()).unwrap();
    let harness = TestHarness::new(scene_handler(scene)).await;

    // Set a low hard cap via spatial_config
    harness
        .call_tool("spatial_config", json!({ "token_hard_cap": 400 }))
        .await
        .unwrap();

    // Snapshot without specifying token_budget — hard cap from config applies
    let result = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    let budget = result.get("budget").expect("budget block must be present");
    let hard_cap = budget["hard_cap"].as_u64().unwrap_or(u64::MAX);
    assert!(
        hard_cap <= 400,
        "hard_cap in snapshot budget should reflect config value of 400, got {hard_cap}: {result}"
    );
}

/// spatial_config persists across multiple subsequent tool calls in the same session.
#[tokio::test]
async fn test_config_persists_across_multiple_calls() {
    let scene = serde_json::to_value(mock_scene_3d()).unwrap();
    let harness = TestHarness::new(scene_handler(scene)).await;

    harness
        .call_tool("spatial_config", json!({ "token_hard_cap": 500 }))
        .await
        .unwrap();

    // Three subsequent calls should all see the same hard cap
    for i in 0..3 {
        let result = harness
            .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
            .await
            .unwrap();

        let hard_cap = result["budget"]["hard_cap"].as_u64().unwrap_or(u64::MAX);
        assert!(
            hard_cap <= 500,
            "call {i}: hard_cap should be ≤ 500, got {hard_cap}: {result}"
        );
    }
}

// ============================================================================
// Section 5: Recording lifecycle consistency
// recording_id returned by start must match status and stop responses
// ============================================================================

/// Start → status → stop: recording_id must be consistent across all three calls.
/// status must show active=true between start and stop.
/// stop must report frames_captured > 0.
#[tokio::test]
async fn test_recording_lifecycle_ids_consistent() {
    // Shared state for the mock: records the active recording id
    let active_id: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
    let aid = active_id.clone();

    let handler: QueryHandler = Arc::new(move |method, _params| {
        let mut id_guard = aid.lock().unwrap();
        match method {
            "recording_start" => {
                let id = "rec_scenario_001".to_string();
                *id_guard = Some(id.clone());
                Ok(json!({
                    "recording_id": id,
                    "name": "scenario_test",
                    "started_at_frame": 100
                }))
            }
            "recording_status" => {
                let active = id_guard.is_some();
                let id = id_guard.clone().unwrap_or_default();
                Ok(json!({
                    "recording_active": active,
                    "recording_id": id,
                    "name": "scenario_test",
                    "frames_captured": if active { 150u32 } else { 0u32 },
                    "duration_ms": 2500u64,
                    "buffer_size_kb": 12u32
                }))
            }
            "recording_stop" => {
                let id = id_guard.take().unwrap_or_default();
                Ok(json!({
                    "recording_id": id,
                    "name": "scenario_test",
                    "frames_captured": 150u32,
                    "duration_ms": 2500u64,
                    "frame_range": [100u64, 250u64]
                }))
            }
            _ => Err(("unknown".into(), format!("unexpected: {method}"))),
        }
    });

    let harness = TestHarness::new(handler).await;

    // Start
    let start = harness
        .call_tool("recording", json!({ "action": "start" }))
        .await
        .unwrap();
    let started_id = start["recording_id"]
        .as_str()
        .expect("start must return recording_id")
        .to_string();
    assert_eq!(started_id, "rec_scenario_001");

    // Status: must show active with the same id
    let status = harness
        .call_tool("recording", json!({ "action": "status" }))
        .await
        .unwrap();
    let status_active = status["recording_active"]
        .as_bool()
        .or_else(|| status["active"].as_bool())
        .unwrap_or(false);
    let status_id = status["recording_id"].as_str().unwrap_or("");
    assert!(status_active, "status should show active=true after start: {status}");
    assert_eq!(
        status_id, started_id,
        "status recording_id must match start id"
    );

    // Stop: must return the same id and a non-zero frame count
    let stop = harness
        .call_tool("recording", json!({ "action": "stop" }))
        .await
        .unwrap();
    let stopped_id = stop["recording_id"].as_str().unwrap_or("");
    let frames = stop["frames_captured"].as_u64().unwrap_or(0);
    assert_eq!(stopped_id, started_id, "stop recording_id must match start id");
    assert!(frames > 0, "stop must report frames_captured > 0: {stop}");

    // Status after stop: must show active=false
    let status_after = harness
        .call_tool("recording", json!({ "action": "status" }))
        .await
        .unwrap();
    let active_after = status_after["recording_active"]
        .as_bool()
        .or_else(|| status_after["active"].as_bool())
        .unwrap_or(true);
    assert!(!active_after, "status should show active=false after stop: {status_after}");
}

// ============================================================================
// Section 6: Error isolation
// A failed tool call must not corrupt state for subsequent calls
// ============================================================================

/// A failed inspect does not corrupt the snapshot path.
/// snapshot → inspect-fails → snapshot must still work.
#[tokio::test]
async fn test_error_isolation_snapshot_survives_failed_inspect() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "get_node_inspect" => Err(("node_not_found".into(), "Node '/Ghost' not found".into())),
        "get_snapshot_data" => Ok(serde_json::to_value(mock_scene_3d()).unwrap()),
        _ => Err(("unknown".into(), format!("unexpected: {method}"))),
    });

    let harness = TestHarness::new(handler).await;

    // First snapshot — should succeed
    let snap1 = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .expect("first snapshot should succeed");
    assert!(snap1.get("entities").is_some());

    // Inspect a missing node — should fail
    let err = harness
        .call_tool("spatial_inspect", json!({ "node": "/Ghost" }))
        .await;
    assert!(err.is_err(), "inspect of missing node should fail");

    // Second snapshot — should still succeed despite the failed inspect
    let snap2 = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .expect("snapshot should succeed after failed inspect");
    assert!(
        snap2.get("entities").is_some(),
        "snapshot should return entities after failed inspect: {snap2}"
    );
}

/// A failed action does not remove existing watches.
#[tokio::test]
async fn test_error_isolation_watches_survive_failed_action() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "execute_action" => {
            Err(("node_not_found".into(), "Node '/Ghost' not found".into()))
        }
        _ => Ok(json!({})),
    });

    let harness = TestHarness::new(handler).await;

    // Add two watches
    let w1 = harness
        .call_tool(
            "spatial_watch",
            json!({ "action": "add", "watch": { "node": "Player", "track": ["state"] } }),
        )
        .await
        .unwrap();
    let id1 = w1["watch_id"].as_str().unwrap().to_string();

    let w2 = harness
        .call_tool(
            "spatial_watch",
            json!({ "action": "add", "watch": { "node": "enemies/Scout", "track": ["state"] } }),
        )
        .await
        .unwrap();
    let id2 = w2["watch_id"].as_str().unwrap().to_string();

    // Failed action on a ghost node
    let _ = harness
        .call_tool(
            "spatial_action",
            json!({ "action": "teleport", "node": "/Ghost", "position": [0.0, 0.0, 0.0] }),
        )
        .await;

    // Both watches must still be listed
    let list = harness
        .call_tool("spatial_watch", json!({ "action": "list" }))
        .await
        .unwrap();
    let watches = list["watches"].as_array().unwrap();
    assert_eq!(watches.len(), 2, "both watches should survive the failed action: {list}");
    // In list responses the id field is "id"; in add responses it's "watch_id"
    let watch_ids: Vec<&str> = watches
        .iter()
        .filter_map(|w| w["id"].as_str().or_else(|| w["watch_id"].as_str()))
        .collect();
    assert!(watch_ids.contains(&id1.as_str()), "watch 1 should still exist");
    assert!(watch_ids.contains(&id2.as_str()), "watch 2 should still exist");
}

/// Multiple successive failures leave the session in a state where success still works.
#[tokio::test]
async fn test_error_isolation_three_failures_then_success() {
    let call_count = Arc::new(AtomicU32::new(0));
    let cc = call_count.clone();

    let handler: QueryHandler = Arc::new(move |method, _| {
        if method == "get_snapshot_data" {
            let n = cc.fetch_add(1, Ordering::SeqCst);
            if n < 3 {
                // First 3 calls fail
                Err(("scene_not_loaded".into(), "Scene loading".into()))
            } else {
                // 4th+ calls succeed
                Ok(serde_json::to_value(mock_scene_3d()).unwrap())
            }
        } else {
            Err(("unknown".into(), format!("unexpected: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;

    // Three successive failures
    for i in 0..3 {
        let err = harness
            .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
            .await;
        assert!(err.is_err(), "call {i} should fail");
    }

    // Fourth call should succeed
    let result = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .expect("snapshot should succeed after 3 failures");
    assert!(
        result.get("entities").is_some(),
        "4th call should return entities: {result}"
    );
}

// ============================================================================
// Section 7: Event ordering — signals arrive via TCP and drain exactly once
// ============================================================================

/// Multiple signals pushed before delta should all appear in one delta call.
#[tokio::test]
async fn test_all_buffered_signals_appear_in_single_delta() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown".into(), format!("unexpected: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;

    // Establish baseline
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Push 3 distinct signal events from mock
    for i in 0..3u32 {
        harness
            .mock
            .push_event(
                "signal_emitted",
                json!({
                    "node": "Player",
                    "signal": format!("event_{i}"),
                    "args": [i],
                    "frame": 100 + i as u64
                }),
            )
            .await;
    }

    // Allow time for events to travel: mock → TCP → server event buffer
    sleep(Duration::from_millis(200)).await;

    let delta = harness.call_tool("spatial_delta", json!({})).await.unwrap();

    let signals = delta["signals_emitted"]
        .as_array()
        .expect("delta must have signals_emitted");
    assert_eq!(
        signals.len(),
        3,
        "all 3 buffered signals should appear in single delta: {delta}"
    );
}

/// Signals are drained by delta: a second delta call should see none of the signals
/// from before the first delta.
#[tokio::test]
async fn test_signals_drained_after_first_delta() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown".into(), format!("unexpected: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;

    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Push 2 signals
    for i in 0..2u32 {
        harness
            .mock
            .push_event(
                "signal_emitted",
                json!({ "node": "enemies/Scout", "signal": format!("hit_{i}"), "args": [], "frame": 200 + i as u64 }),
            )
            .await;
    }
    sleep(Duration::from_millis(200)).await;

    // First delta: drains the 2 signals
    let d1 = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    let count1 = d1["signals_emitted"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(count1, 2, "first delta should see 2 signals: {d1}");

    // Second delta: no new signals pushed → should be empty
    let d2 = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    let count2 = d2["signals_emitted"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(
        count2, 0,
        "second delta should see 0 signals (already drained): {d2}"
    );
}

/// Signals pushed after a delta appear only in the NEXT delta, not retroactively.
#[tokio::test]
async fn test_signals_pushed_after_delta_appear_in_next_delta() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown".into(), format!("unexpected: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Delta 1: no signals yet — should be empty
    let d1 = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    assert_eq!(
        d1["signals_emitted"].as_array().map(|a| a.len()).unwrap_or(0),
        0,
        "delta before any signals should have 0 signals: {d1}"
    );

    // Push a signal AFTER the first delta
    harness
        .mock
        .push_event(
            "signal_emitted",
            json!({ "node": "Player", "signal": "late_event", "args": [], "frame": 300 }),
        )
        .await;
    sleep(Duration::from_millis(200)).await;

    // Delta 2: should see the late signal
    let d2 = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    assert_eq!(
        d2["signals_emitted"].as_array().map(|a| a.len()).unwrap_or(0),
        1,
        "delta after signal push should see 1 signal: {d2}"
    );
}

// ============================================================================
// Section 8: Watch + snapshot + delta — three-tool interaction
// ============================================================================

/// Watches added, then cleared: events pushed after clear should not appear
/// as watch-attributed events in delta.
///
/// Note: signal_emitted events in delta["signals_emitted"] are not filtered by
/// watches — they reflect ALL events from the addon. This test verifies that
/// watches can be managed (add/clear) independently of the event stream.
#[tokio::test]
async fn test_watch_cleared_before_event_does_not_affect_signal_stream() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown".into(), format!("unexpected: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Add 3 watches
    for node in ["Player", "enemies/Scout", "items/Coin"] {
        harness
            .call_tool(
                "spatial_watch",
                json!({ "action": "add", "watch": { "node": node, "track": ["state"] } }),
            )
            .await
            .unwrap();
    }
    let list = harness
        .call_tool("spatial_watch", json!({ "action": "list" }))
        .await
        .unwrap();
    assert_eq!(list["watches"].as_array().unwrap().len(), 3);

    // Clear all watches
    harness
        .call_tool("spatial_watch", json!({ "action": "clear" }))
        .await
        .unwrap();
    let list_after = harness
        .call_tool("spatial_watch", json!({ "action": "list" }))
        .await
        .unwrap();
    assert_eq!(list_after["watches"].as_array().unwrap().len(), 0, "watches should be empty after clear");

    // Signals still arrive (the event stream is independent of watches)
    harness
        .mock
        .push_event(
            "signal_emitted",
            json!({ "node": "Player", "signal": "health_changed", "args": [50], "frame": 400 }),
        )
        .await;
    sleep(Duration::from_millis(200)).await;

    let delta = harness.call_tool("spatial_delta", json!({})).await.unwrap();
    // Signals still appear in the event stream even with no watches
    let signals = delta["signals_emitted"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(signals, 1, "signal event stream should be independent of watches: {delta}");
}

/// After clearing watches, re-adding them works correctly.
#[tokio::test]
async fn test_watches_can_be_re_added_after_clear() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    // Add, then clear
    harness
        .call_tool(
            "spatial_watch",
            json!({ "action": "add", "watch": { "node": "Player", "track": ["state"] } }),
        )
        .await
        .unwrap();
    harness
        .call_tool("spatial_watch", json!({ "action": "clear" }))
        .await
        .unwrap();

    // Re-add
    let w = harness
        .call_tool(
            "spatial_watch",
            json!({ "action": "add", "watch": { "node": "enemies/Scout", "track": ["state"] } }),
        )
        .await
        .unwrap();
    let id = w["watch_id"].as_str().expect("must return watch_id");

    let list = harness
        .call_tool("spatial_watch", json!({ "action": "list" }))
        .await
        .unwrap();
    let watches = list["watches"].as_array().unwrap();
    assert_eq!(watches.len(), 1, "should have exactly 1 watch after re-add");
    // In list responses the id field is "id"; in add responses it's "watch_id"
    assert!(
        watches
            .iter()
            .any(|w| w["id"].as_str() == Some(id) || w["watch_id"].as_str() == Some(id)),
        "re-added watch should appear in list: {list}"
    );
}

// ============================================================================
// Section 9: Inspect ↔ snapshot coherence
// An entity returned by snapshot should be inspectable
// ============================================================================

/// Snapshot returns entity paths; inspect on one of those paths should succeed.
#[tokio::test]
async fn test_inspect_entity_returned_by_snapshot() {
    use spectator_protocol::query::NodeInspectResponse;

    let inspect_response = NodeInspectResponse {
        path: "enemies/Scout".into(),
        class: "CharacterBody3D".into(),
        instance_id: 99001,
        transform: None,
        physics: None,
        state: None,
        children: None,
        signals: None,
        script: None,
        spatial_context_raw: None,
    };
    let inspect_value = Arc::new(serde_json::to_value(&inspect_response).unwrap());
    let iv = inspect_value.clone();

    let handler: QueryHandler = Arc::new(move |method, params| match method {
        "get_snapshot_data" => Ok(serde_json::to_value(mock_scene_3d()).unwrap()),
        "get_node_inspect" => {
            let node = params["path"].as_str().unwrap_or("");
            if node.contains("Scout") || node.contains("enemies") {
                Ok((*iv).clone())
            } else {
                Err(("node_not_found".into(), format!("not found: {node}")))
            }
        }
        _ => Err(("unknown".into(), format!("unexpected: {method}"))),
    });

    let harness = TestHarness::new(handler).await;

    // Snapshot to find entities
    let snap = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();
    let entities = snap["entities"].as_array().unwrap();

    // Find Scout in the snapshot
    let scout = entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false))
        .expect("Scout should be in snapshot");
    let path = scout["path"].as_str().unwrap();

    // Inspect Scout using the path from the snapshot
    let inspect = harness
        .call_tool("spatial_inspect", json!({ "node": path }))
        .await
        .expect("inspect of entity from snapshot should succeed");

    assert_eq!(inspect["path"], "enemies/Scout");
    assert_eq!(inspect["class"], "CharacterBody3D");
}
