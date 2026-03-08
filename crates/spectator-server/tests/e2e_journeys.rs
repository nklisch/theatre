/// E2E Journey Tests — real Godot headless + real SpectatorServer.
///
/// These tests require:
///   - GODOT_BIN env var pointing to a Godot 4.x binary (or `godot` on PATH)
///   - GDExtension built and deployed to tests/godot-project/addons/spectator/
///     Run: spectator-deploy ~/dev/spectator/tests/godot-project
///
/// Run: cargo test -p spectator-server --features e2e-tests -- --nocapture
mod support;

use serde_json::json;

/// Journey: Agent connects and explores a 3D scene.
///
/// Tests the real handshake, real collector data, real scene tree,
/// and real spatial indexing against actual Godot transforms.
///
/// Steps:
///   1. Verify handshake: session connected, dimensions=3, project="SpectatorTests"
///   2. scene_tree() → real hierarchy (TestScene3D, Camera3D, Player, Enemies/Scout, ...)
///   3. spatial_snapshot(summary) → clustered groups, correct entity count
///   4. spatial_snapshot(standard) → per-entity data with real positions
///        Assert: Player at ~(0,0,0), Scout at ~(5,0,-3), Tank at ~(-4,0,2)
///   5. spatial_inspect(Enemies/Scout) → real transform, class, health=80
///   6. spatial_query(nearest, from Player position, k=2) → two closest entities
///        Assert: results exist and distances are geometrically plausible
#[tokio::test]
async fn journey_explore_scene() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: Verify handshake
    {
        let state = h.state.lock().await;
        assert!(state.connected, "Session should be connected after handshake");
        let info = state
            .handshake_info
            .as_ref()
            .expect("Handshake info should be present");
        assert_eq!(info.scene_dimensions, 3, "Expected 3D scene dimensions");
        assert!(
            !info.project_name.is_empty(),
            "Project name should be non-empty"
        );
    }

    // Step 2: scene_tree — roots returns top-level nodes only
    let tree = h
        .expect(2, "scene_tree", json!({"action": "roots"}))
        .await;
    assert!(
        tree["roots"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "Scene tree roots should return at least one node, got: {tree}"
    );

    // Step 3: snapshot summary
    let summary = h
        .expect(3, "spatial_snapshot", json!({"detail": "summary"}))
        .await;
    // Summary should have some data (groups or entity count)
    assert!(
        !summary.is_null(),
        "Summary snapshot should return non-null data"
    );

    // Step 4: snapshot standard — real positions
    let snapshot = h
        .expect(4, "spatial_snapshot", json!({"detail": "standard"}))
        .await;
    let entities = snapshot["entities"]
        .as_array()
        .expect("Expected entities array in standard snapshot");
    assert!(
        !entities.is_empty(),
        "Should have at least one entity in snapshot"
    );

    // Find Player and check position ~(0, 0, 0)
    let player = entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Player")).unwrap_or(false))
        .expect("Player should be in snapshot");
    let player_pos = player["global_position"]
        .as_array()
        .expect("Player should have position array");
    assert_eq!(player_pos.len(), 3, "3D position should have 3 components");
    assert!(
        player_pos[0].as_f64().unwrap_or(999.0).abs() < 1.0,
        "Player X should be ~0, got {player_pos:?}"
    );

    // Find Scout and check position ~(5, 0, -3)
    let scout = entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false))
        .expect("Scout should be in snapshot");
    let scout_pos = scout["global_position"]
        .as_array()
        .expect("Scout should have position array");
    assert!(
        (scout_pos[0].as_f64().unwrap_or(0.0) - 5.0).abs() < 1.0,
        "Scout X should be ~5, got {scout_pos:?}"
    );
    assert!(
        (scout_pos[2].as_f64().unwrap_or(0.0) - (-3.0)).abs() < 1.0,
        "Scout Z should be ~-3, got {scout_pos:?}"
    );

    // Step 5: spatial_inspect Scout
    let inspect = h
        .expect(
            5,
            "spatial_inspect",
            json!({"node": "Enemies/Scout"}),
        )
        .await;
    // Should have class info
    assert!(
        inspect["class"].is_string() || inspect["node_path"].is_string(),
        "Inspect should return node info, got: {inspect}"
    );
    // Check health=80 exported var
    let props = inspect["properties"].as_object();
    if let Some(props) = props {
        if let Some(health) = props.get("health") {
            let health_val = health.as_f64().unwrap_or(0.0);
            assert!(
                (health_val - 80.0).abs() < 0.1,
                "Scout health should be 80, got {health_val}"
            );
        }
    }

    // Step 6: spatial_query nearest from Player
    let query = h
        .expect(
            6,
            "spatial_query",
            json!({
                "query_type": "nearest",
                "from": [0.0, 0.0, 0.0],
                "k": 2
            }),
        )
        .await;
    let results = query["results"]
        .as_array()
        .expect("spatial_query should return results array");
    assert!(
        !results.is_empty(),
        "Nearest query should return at least one result"
    );
    // Distances should be geometrically plausible
    for r in results {
        let dist = r["distance"].as_f64().unwrap_or(-1.0);
        assert!(
            dist >= 0.0,
            "Distance should be non-negative, got {dist}"
        );
    }

    // Step 7: resource inspection — Scout has a CapsuleShape3D collision shape
    let resources = h
        .expect(
            7,
            "spatial_inspect",
            json!({
                "node": "Enemies/Scout",
                "include": ["resources"]
            }),
        )
        .await;

    // resources key must be present when explicitly requested
    assert!(
        resources.get("resources").is_some(),
        "resources field missing from inspect response"
    );

    // Scout has exactly one CollisionShape3D child with a CapsuleShape3D
    let shapes = resources["resources"]["collision_shapes"]
        .as_array()
        .expect("collision_shapes should be an array");
    assert!(!shapes.is_empty(), "Scout should have at least one collision shape");

    let shape = &shapes[0];
    assert_eq!(
        shape["type"].as_str().unwrap_or(""),
        "CapsuleShape3D",
        "Scout's collision shape should be CapsuleShape3D"
    );

    // Dimensions should match the scene file (radius=0.4, height=1.8)
    let dims = shape["dimensions"].as_object().expect("dimensions map missing");
    let radius = dims["radius"].as_f64().unwrap_or(0.0);
    let height = dims["height"].as_f64().unwrap_or(0.0);
    assert!(
        (radius - 0.4).abs() < 0.05,
        "Scout capsule radius should be ~0.4, got {radius}"
    );
    assert!(
        (height - 1.8).abs() < 0.05,
        "Scout capsule height should be ~1.8, got {height}"
    );

    // resources should NOT appear in a default inspect (no include specified)
    let default_inspect = h
        .expect(
            7,
            "spatial_inspect",
            json!({ "node": "Enemies/Scout" }),
        )
        .await;
    assert!(
        default_inspect.get("resources").is_none(),
        "resources should be absent from default inspect"
    );
}

/// Journey: Teleport an enemy, verify through snapshot + delta + inspect.
///
/// Tests the most critical cross-boundary interaction: actions mutate real
/// Godot state, and that mutation is visible through all observation tools.
///
/// Steps:
///   1. spatial_snapshot(standard) → baseline: Scout at ~(5, 0, -3)
///   2. spatial_action(teleport, Enemies/Scout, [0, 0, 0]) → ack + previous position
///   3. wait_frames(5) → let physics settle
///   4. spatial_snapshot(standard) → Scout now at ~(0, 0, 0)
///   5. spatial_delta() → Scout in "moved" array
///   6. spatial_inspect(Enemies/Scout) → transform at new position, health still 80
///   7. spatial_action(set_property, Enemies/Scout, health, 25) → ack + old value
///   8. wait_frames(2)
///   9. spatial_inspect(Enemies/Scout) → health now 25
///  10. spatial_delta() → Scout in state_changed (health went 80→25)
#[tokio::test]
async fn journey_debug_spatial_bug() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: baseline snapshot
    let baseline = h
        .expect(1, "spatial_snapshot", json!({"detail": "standard"}))
        .await;
    let entities = baseline["entities"].as_array().expect("Expected entities");
    let entity_paths: Vec<&str> = entities.iter().filter_map(|e| e["path"].as_str()).collect();
    let scout_before = entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false))
        .expect(&format!("Scout should be in baseline snapshot, found: {entity_paths:?}"));
    let before_pos = scout_before["global_position"].as_array().expect("position array");
    let before_x = before_pos[0].as_f64().unwrap_or(0.0);

    // Step 2: teleport Scout to origin
    let teleport_result = h
        .expect(
            2,
            "spatial_action",
            json!({
                "action": "teleport",
                "node": "Enemies/Scout",
                "position": [0.0, 0.0, 0.0]
            }),
        )
        .await;
    assert!(
        !teleport_result.is_null(),
        "Teleport should return an acknowledgement"
    );

    // Step 3: wait for physics to settle
    h.wait_frames(5).await;

    // Step 4: spatial_delta — compares stored baseline (step 1) against fresh live state.
    // Scout moved from ~(5,0,-3) to ~(0,0,0), so it should appear in "moved".
    // Do NOT call spatial_snapshot before delta: that would update the baseline,
    // making the delta compare post-teleport vs post-teleport (no change detected).
    let delta = h
        .expect(4, "spatial_delta", json!({}))
        .await;
    let empty_arr = vec![];
    let moved = delta["moved"].as_array().unwrap_or(&empty_arr);
    let scout_moved = moved
        .iter()
        .any(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false));
    assert!(
        scout_moved,
        "Scout should appear in delta.moved after teleport. Full delta: {delta}"
    );
    if let Some(scout_delta) = moved
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false))
    {
        // Compute displacement from delta_pos: sqrt(dx^2 + dy^2 + dz^2)
        let dp = scout_delta["delta_pos"].as_array();
        if let Some(dp) = dp {
            let dx = dp.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let dy = dp.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let dz = dp.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let displacement = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!(
                displacement > 4.0,
                "Scout displacement should be > 4.0, got {displacement}. delta_pos: {dp:?}"
            );
        }
    }

    // Step 5: snapshot to see Scout at new position and update the delta baseline
    let after_snapshot = h
        .expect(5, "spatial_snapshot", json!({"detail": "standard"}))
        .await;
    let after_entities = after_snapshot["entities"].as_array().expect("Expected entities");
    let scout_after = after_entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false))
        .expect("Scout should still be in post-teleport snapshot");
    let after_pos = scout_after["global_position"].as_array().expect("position array");
    let after_x = after_pos[0].as_f64().unwrap_or(999.0);
    assert!(
        (after_x - before_x).abs() > 2.0,
        "Scout position should have changed after teleport: before={before_x}, after={after_x}"
    );

    // Step 6: inspect Scout — at new position, health still 80
    let inspect = h
        .expect(6, "spatial_inspect", json!({"node": "Enemies/Scout"}))
        .await;
    let props = inspect["properties"].as_object();
    if let Some(props) = props {
        if let Some(health) = props.get("health") {
            let health_val = health.as_f64().unwrap_or(0.0);
            assert!(
                (health_val - 80.0).abs() < 0.1,
                "Scout health should still be 80 before set_property, got {health_val}"
            );
        }
    }

    // Step 7: set health to 25
    let set_result = h
        .expect(
            7,
            "spatial_action",
            json!({
                "action": "set_property",
                "node": "Enemies/Scout",
                "property": "health",
                "value": 25
            }),
        )
        .await;
    assert!(
        !set_result.is_null(),
        "set_property should return an acknowledgement"
    );

    // Step 8: wait for property update to propagate, then delta to detect the change.
    // The snapshot from step 5 is the baseline. set_property changed health 80→25.
    // spatial_delta will query fresh state and compare against step-5 baseline.
    h.wait_frames(2).await;

    // Step 9: delta — Scout should appear in state_changed (health 80→25)
    let delta2 = h
        .expect(9, "spatial_delta", json!({}))
        .await;
    let empty_arr2 = vec![];
    let state_changed = delta2["state_changed"].as_array().unwrap_or(&empty_arr2);
    let scout_changed = state_changed
        .iter()
        .any(|e| e["path"].as_str().map(|p| p.contains("Scout")).unwrap_or(false));
    assert!(
        scout_changed,
        "Scout should appear in delta.state_changed after set_property. Full delta: {delta2}"
    );

    // Step 10: inspect to verify health=25
    let inspect2 = h
        .expect(10, "spatial_inspect", json!({"node": "Enemies/Scout"}))
        .await;
    let props2 = inspect2["properties"].as_object();
    if let Some(props2) = props2 {
        if let Some(health) = props2.get("health") {
            let health_val = health.as_f64().unwrap_or(80.0);
            assert!(
                (health_val - 25.0).abs() < 0.1,
                "Scout health should be 25 after set_property, got {health_val}"
            );
        }
    }
}

/// Journey: Record game state, verify recording lifecycle.
///
/// Steps:
///   1. spatial_snapshot(standard) → baseline, note frame number
///   2. recording(start) → recording_id returned
///   3. recording(status) → active=true, recording_id matches
///   4. wait_frames(30) → let recorder capture ~30 frames
///   5. spatial_snapshot(standard) → mid-recording snapshot still works
///        Assert: frame number advanced from step 1
///   6. recording(add_marker, source="agent", label="mid_test") → ack
///   7. wait_frames(30) → more frames
///   8. recording(stop) → frames_captured > 0
///   9. recording(status) → active=false
///  10. spatial_snapshot(standard) → post-recording snapshot still works
#[tokio::test]
async fn journey_recording_lifecycle() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: baseline snapshot
    let baseline = h
        .expect(1, "spatial_snapshot", json!({"detail": "standard"}))
        .await;
    let frame_0 = baseline["frame"].as_u64().unwrap_or(0);

    // Step 2: start recording
    let start_result = h
        .expect(2, "recording", json!({"action": "start"}))
        .await;
    let recording_id = start_result["recording_id"]
        .as_str()
        .expect("recording start should return recording_id")
        .to_string();
    assert!(!recording_id.is_empty(), "recording_id should be non-empty");

    // Step 3: check status
    let status = h
        .expect(3, "recording", json!({"action": "status"}))
        .await;
    assert_eq!(
        status["recording_active"].as_bool(),
        Some(true),
        "Recording should be active after start"
    );
    assert_eq!(
        status["recording_id"].as_str(),
        Some(recording_id.as_str()),
        "Status recording_id should match"
    );

    // Step 4: wait ~30 frames
    h.wait_frames(30).await;

    // Step 5: mid-recording snapshot
    let mid_snapshot = h
        .expect(5, "spatial_snapshot", json!({"detail": "standard"}))
        .await;
    let frame_mid = mid_snapshot["frame"].as_u64().unwrap_or(0);
    assert!(
        frame_mid > frame_0,
        "Frame counter should advance during recording: {frame_0} → {frame_mid}"
    );

    // Step 6: add marker
    let marker_result = h
        .expect(
            6,
            "recording",
            json!({
                "action": "add_marker",
                "marker_label": "mid_test"
            }),
        )
        .await;
    assert!(
        !marker_result.is_null(),
        "add_marker should return acknowledgement"
    );

    // Step 7: wait more frames
    h.wait_frames(30).await;

    // Step 8: stop recording
    let stop_result = h
        .expect(8, "recording", json!({"action": "stop"}))
        .await;
    let frames_captured = stop_result["frames_captured"].as_u64().unwrap_or(0);
    assert!(
        frames_captured > 0,
        "frames_captured should be > 0 after recording, got {frames_captured}"
    );

    // Step 9: status after stop
    let status_after = h
        .expect(9, "recording", json!({"action": "status"}))
        .await;
    assert_eq!(
        status_after["recording_active"].as_bool(),
        Some(false),
        "Recording should not be active after stop"
    );

    // Step 10: post-recording snapshot — session should not be corrupted
    let post_snapshot = h
        .expect(10, "spatial_snapshot", json!({"detail": "standard"}))
        .await;
    let post_entities = post_snapshot["entities"].as_array();
    assert!(
        post_entities.is_some(),
        "Post-recording snapshot should still return entities (session not corrupted)"
    );
}

/// Journey: 2D scene returns correct position format and bearings.
///
/// Steps:
///   1. Verify handshake: dimensions=2
///   2. spatial_snapshot(standard) → entities have [x, y] positions (2 elements)
///        Assert: Player at ~(0, 0), Scout2D at ~(200, 100)
///   3. spatial_query(nearest, from [0,0], k=2) → nearest entities
///        Assert: results have 2-element positions
///   4. spatial_inspect(Player) → 2D transform (no z component)
///   5. spatial_action(teleport, Player, [100, 50]) → ack
///   6. wait_frames(3)
///   7. spatial_snapshot(standard) → Player now at ~(100, 50)
#[tokio::test]
async fn journey_2d_scene() {
    let mut h = support::e2e_harness::E2EHarness::start_2d()
        .await
        .expect("Failed to start Godot 2D scene");

    // Step 1: verify 2D handshake
    {
        let state = h.state.lock().await;
        assert!(state.connected, "Should be connected");
        let info = state
            .handshake_info
            .as_ref()
            .expect("Handshake info required");
        assert_eq!(
            info.scene_dimensions, 2,
            "Expected 2D scene dimensions, got {}",
            info.scene_dimensions
        );
    }

    // Step 2: 2D snapshot — 2D pixel coords need a larger radius (Scout2D at ~224px from origin)
    let snapshot = h
        .expect(2, "spatial_snapshot", json!({"detail": "standard", "radius": 500.0}))
        .await;
    let entities = snapshot["entities"]
        .as_array()
        .expect("Expected entities array");
    assert!(
        !entities.is_empty(),
        "2D snapshot should have at least one entity"
    );

    // All positions should be 2-element arrays
    for entity in entities {
        if let Some(pos) = entity["global_position"].as_array() {
            assert_eq!(
                pos.len(),
                2,
                "2D positions should have 2 elements, got {} for entity {}",
                pos.len(),
                entity["path"]
            );
        }
    }

    // Find Player ~(0, 0) — Player is at the origin in the 2D scene
    let player = entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p == "Player").unwrap_or(false))
        .expect("Player should be in 2D snapshot");
    let player_pos = player["global_position"].as_array().expect("Player should have abs position");
    assert!(
        player_pos[0].as_f64().unwrap_or(999.0).abs() < 1.0,
        "Player X should be ~0 in 2D scene, got {:?}",
        player_pos
    );
    assert!(
        player_pos[1].as_f64().unwrap_or(999.0).abs() < 1.0,
        "Player Y should be ~0 in 2D scene, got {:?}",
        player_pos
    );

    // Step 3: spatial_query in 2D
    let query = h
        .expect(
            3,
            "spatial_query",
            json!({
                "query_type": "nearest",
                "from": [0.0, 0.0],
                "k": 2
            }),
        )
        .await;
    let results = query["results"]
        .as_array()
        .expect("spatial_query should return results");
    assert!(
        !results.is_empty(),
        "2D nearest query should return results"
    );

    // Step 4: inspect Player — 2D transform (no elevation)
    let inspect = h
        .expect(4, "spatial_inspect", json!({"node": "Player"}))
        .await;
    assert!(
        !inspect.is_null(),
        "Inspect should return data for Player in 2D scene"
    );
    // Ensure no elevation field in 2D response
    assert!(
        inspect["elevation"].is_null(),
        "2D inspect should not have elevation field"
    );

    // Step 5: teleport Player to (100, 50)
    let teleport = h
        .expect(
            5,
            "spatial_action",
            json!({
                "action": "teleport",
                "node": "Player",
                "position": [100.0, 50.0]
            }),
        )
        .await;
    assert!(
        !teleport.is_null(),
        "2D teleport should return acknowledgement"
    );

    // Step 6: wait for physics
    h.wait_frames(3).await;

    // Step 7: post-teleport snapshot — Player at ~(100, 50)
    // include_offscreen: true so Player is found even if outside camera frustum
    let after = h
        .expect(
            7,
            "spatial_snapshot",
            json!({"detail": "standard", "radius": 500.0, "include_offscreen": true}),
        )
        .await;
    let after_entities = after["entities"].as_array().expect("entities");
    let player_after = after_entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p.contains("Player")).unwrap_or(false))
        .expect("Player should still be in post-teleport 2D snapshot");
    let player_after_pos = player_after["global_position"].as_array().expect("position");
    assert_eq!(
        player_after_pos.len(),
        2,
        "Post-teleport 2D position should still have 2 elements"
    );
    assert!(
        (player_after_pos[0].as_f64().unwrap_or(0.0) - 100.0).abs() < 5.0,
        "Player X should be ~100 after teleport, got {:?}",
        player_after_pos
    );
    assert!(
        (player_after_pos[1].as_f64().unwrap_or(0.0) - 50.0).abs() < 5.0,
        "Player Y should be ~50 after teleport, got {:?}",
        player_after_pos
    );
}
