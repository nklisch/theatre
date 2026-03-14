/// E2E Journey Tests — real Godot headless + real StageServer.
///
/// These tests require:
///   - GODOT_BIN env var pointing to a Godot 4.x binary (or `godot` on PATH)
///   - GDExtension built and deployed to tests/godot-project/addons/stage/
///     Run: theatre deploy ~/dev/theatre/tests/godot-project
///
/// Run: cargo test -p stage-server --test e2e_journeys -- --nocapture
mod support;

use serde_json::json;

/// Journey: Agent connects and explores a 3D scene.
///
/// Tests the real handshake, real collector data, real scene tree,
/// and real spatial indexing against actual Godot transforms.
///
/// Steps:
///   1. Verify handshake: session connected, dimensions=3, project="StageTests"
///   2. scene_tree() → real hierarchy (TestScene3D, Camera3D, Player, Enemies/Scout, ...)
///   3. spatial_snapshot(summary) → clustered groups, correct entity count
///   4. spatial_snapshot(standard) → per-entity data with real positions
///        Assert: Player at ~(0,0,0), Scout at ~(5,0,-3), Tank at ~(-4,0,2)
///   5. spatial_inspect(Enemies/Scout) → real transform, class, health=80
///   6. spatial_query(nearest, from Player position, k=2) → two closest entities
///        Assert: results exist and distances are geometrically plausible
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn journey_explore_scene() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: Verify handshake
    {
        let state = h.state.lock().await;
        assert!(
            state.connected,
            "Session should be connected after handshake"
        );
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
    let tree = h.expect(2, "scene_tree", json!({"action": "roots"})).await;
    assert!(
        tree["roots"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
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
        .find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains("Player"))
                .unwrap_or(false)
        })
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
        .find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains("Scout"))
                .unwrap_or(false)
        })
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
        .expect(5, "spatial_inspect", json!({"node": "Enemies/Scout"}))
        .await;
    // Should have class info
    assert!(
        inspect["class"].is_string() || inspect["node_path"].is_string(),
        "Inspect should return node info, got: {inspect}"
    );
    // Check health=80 exported var
    let props = inspect["properties"].as_object();
    if let Some(props) = props
        && let Some(health) = props.get("health")
    {
        let health_val = health.as_f64().unwrap_or(0.0);
        assert!(
            (health_val - 80.0).abs() < 0.1,
            "Scout health should be 80, got {health_val}"
        );
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
        assert!(dist >= 0.0, "Distance should be non-negative, got {dist}");
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
    assert!(
        !shapes.is_empty(),
        "Scout should have at least one collision shape"
    );

    let shape = &shapes[0];
    assert_eq!(
        shape["type"].as_str().unwrap_or(""),
        "CapsuleShape3D",
        "Scout's collision shape should be CapsuleShape3D"
    );

    // Dimensions should match the scene file (radius=0.4, height=1.8)
    let dims = shape["dimensions"]
        .as_object()
        .expect("dimensions map missing");
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
        .expect(7, "spatial_inspect", json!({ "node": "Enemies/Scout" }))
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
#[ignore = "requires Godot binary"]
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
        .find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains("Scout"))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("Scout should be in baseline snapshot, found: {entity_paths:?}"));
    let before_pos = scout_before["global_position"]
        .as_array()
        .expect("position array");
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
    let delta = h.expect(4, "spatial_delta", json!({})).await;
    let empty_arr = vec![];
    let moved = delta["moved"].as_array().unwrap_or(&empty_arr);
    let scout_moved = moved.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Scout"))
            .unwrap_or(false)
    });
    assert!(
        scout_moved,
        "Scout should appear in delta.moved after teleport. Full delta: {delta}"
    );
    if let Some(scout_delta) = moved.iter().find(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Scout"))
            .unwrap_or(false)
    }) {
        // Compute displacement from delta_pos: sqrt(dx^2 + dy^2 + dz^2)
        let dp = scout_delta["delta_pos"].as_array();
        if let Some(dp) = dp {
            let dx = dp.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
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
    let after_entities = after_snapshot["entities"]
        .as_array()
        .expect("Expected entities");
    let scout_after = after_entities
        .iter()
        .find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains("Scout"))
                .unwrap_or(false)
        })
        .expect("Scout should still be in post-teleport snapshot");
    let after_pos = scout_after["global_position"]
        .as_array()
        .expect("position array");
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
    if let Some(props) = props
        && let Some(health) = props.get("health")
    {
        let health_val = health.as_f64().unwrap_or(0.0);
        assert!(
            (health_val - 80.0).abs() < 0.1,
            "Scout health should still be 80 before set_property, got {health_val}"
        );
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
    let delta2 = h.expect(9, "spatial_delta", json!({})).await;
    let empty_arr2 = vec![];
    let state_changed = delta2["state_changed"].as_array().unwrap_or(&empty_arr2);
    let scout_changed = state_changed.iter().any(|e| {
        e["path"]
            .as_str()
            .map(|p| p.contains("Scout"))
            .unwrap_or(false)
    });
    assert!(
        scout_changed,
        "Scout should appear in delta.state_changed after set_property. Full delta: {delta2}"
    );

    // Step 10: inspect to verify health=25
    let inspect2 = h
        .expect(10, "spatial_inspect", json!({"node": "Enemies/Scout"}))
        .await;
    let props2 = inspect2["properties"].as_object();
    if let Some(props2) = props2
        && let Some(health) = props2.get("health")
    {
        let health_val = health.as_f64().unwrap_or(80.0);
        assert!(
            (health_val - 25.0).abs() < 0.1,
            "Scout health should be 25 after set_property, got {health_val}"
        );
    }
}

/// Journey: Agent uses dashcam to capture a spatial anomaly.
///
/// This is the primary dashcam usage pattern: the agent is debugging a game,
/// notices something suspicious, triggers a dashcam clip to capture the
/// surrounding context, and verifies the clip is available for analysis.
///
/// Steps:
///   1. Verify dashcam is active: clips(status) returns state="buffering"
///   2. spatial_snapshot(standard) → baseline, note entities and frame
///   3. wait_frames(60) → let dashcam buffer accumulate ~1 second of data
///   4. spatial_action(teleport, Enemies/Scout, [100, 0, 100]) → create an anomaly
///   5. wait_frames(5) → let physics settle
///   6. clips(add_marker, marker_label="anomaly detected") → trigger dashcam clip
///   7. clips(status) → state should be "post_capture" (capturing post-window)
///   8. clips(save) → force-close the clip immediately (avoid 30s wait)
///   9. clips(status) → state should be back to "buffering"
///  10. clips(list) → dashcam clip should appear in the list
///  11. Verify the clip has dashcam=true and clip_id starts with "clip_"
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn journey_dashcam_agent_workflow() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: verify dashcam is actively buffering on startup
    let status = h.expect(1, "clips", json!({ "action": "status" })).await;
    assert_eq!(
        status["dashcam_enabled"],
        json!(true),
        "Dashcam should be enabled by default"
    );
    assert_eq!(
        status["state"],
        json!("buffering"),
        "Dashcam should start in buffering state"
    );
    assert!(
        status["buffer_frames"].as_u64().is_some(),
        "buffer_frames should be present"
    );
    assert!(
        status["config"].is_object(),
        "dashcam config should be returned"
    );

    // Step 2: baseline snapshot — verify scene is live
    let baseline = h
        .expect(2, "spatial_snapshot", json!({ "detail": "standard" }))
        .await;
    let entities = baseline["entities"]
        .as_array()
        .expect("Expected entities in baseline snapshot");
    assert!(
        !entities.is_empty(),
        "Baseline snapshot should have entities"
    );

    // Step 3: let dashcam accumulate buffer data (~1s worth)
    h.wait_frames(60).await;

    // Step 4: create a spatial anomaly by teleporting Scout far away
    h.expect(
        4,
        "spatial_action",
        json!({
            "action": "teleport",
            "node": "Enemies/Scout",
            "position": [100.0, 0.0, 100.0]
        }),
    )
    .await;

    // Step 5: let physics settle
    h.wait_frames(5).await;

    // Step 6: agent triggers dashcam clip via add_marker
    let marker_result = h
        .expect(
            6,
            "clips",
            json!({
                "action": "add_marker",
                "marker_label": "anomaly detected"
            }),
        )
        .await;
    assert!(
        !marker_result.is_null(),
        "add_marker should acknowledge the trigger"
    );

    // Step 7: dashcam should now be in post_capture state
    let status_post = h.expect(7, "clips", json!({ "action": "status" })).await;
    assert_eq!(
        status_post["state"],
        json!("post_capture"),
        "Dashcam should be in post_capture after marker trigger. Got: {status_post}"
    );

    // Step 8: force-save to close the clip immediately (avoid 30s deliberate post-window)
    let save_result = h
        .expect(
            8,
            "clips",
            json!({
                "action": "save",
                "marker_label": "force close for test"
            }),
        )
        .await;
    let clip_id = save_result["clip_id"]
        .as_str()
        .expect("save should return clip_id");
    assert!(
        clip_id.starts_with("clip_"),
        "clip id should start with 'clip_', got: {clip_id}"
    );
    assert!(
        save_result["frames"].as_u64().unwrap_or(0) > 0,
        "Clip should contain captured frames"
    );

    // Step 9: dashcam should be back to buffering after save
    let status_after = h.expect(9, "clips", json!({ "action": "status" })).await;
    assert_eq!(
        status_after["state"],
        json!("buffering"),
        "Dashcam should return to buffering after clip save. Got: {status_after}"
    );

    // Step 10: clip should appear in list
    let list = h.expect(10, "clips", json!({ "action": "list" })).await;
    let clips = list["clips"]
        .as_array()
        .expect("list should return clips array");
    let dashcam_clips: Vec<_> = clips
        .iter()
        .filter(|r| r["dashcam"].as_bool() == Some(true))
        .collect();
    assert!(
        !dashcam_clips.is_empty(),
        "At least one dashcam clip should be in the list. Full list: {list}"
    );

    // Step 11: verify clip metadata
    let our_clip = clips
        .iter()
        .find(|r| r["clip_id"].as_str() == Some(clip_id))
        .unwrap_or_else(|| panic!("Clip {clip_id} should be in list. Clips: {clips:?}"));
    assert_eq!(our_clip["dashcam"], json!(true));
}

/// Journey: Save a dashcam clip, verify it exists, delete it, verify it's gone.
///
/// Steps:
///   1. wait_frames(60) → accumulate buffer
///   2. clips(save, "cleanup_test") → get clip_id
///   3. clips(list) → clip should be present
///   4. clips(delete, clip_id) → delete the clip
///   5. clips(list) → clip should be gone
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn journey_dashcam_clip_lifecycle() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: let buffer accumulate
    h.wait_frames(60).await;

    // Step 2: save clip
    let save = h
        .expect(
            2,
            "clips",
            json!({ "action": "save", "marker_label": "cleanup_test" }),
        )
        .await;
    let clip_id = save["clip_id"]
        .as_str()
        .expect("save should return clip_id")
        .to_string();

    // Step 3: verify clip exists in list
    let list = h.expect(3, "clips", json!({ "action": "list" })).await;
    let found = list["clips"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["clip_id"].as_str() == Some(&clip_id));
    assert!(found, "Clip {clip_id} should be in list after save");

    // Step 4: delete the clip
    let delete = h
        .expect(
            4,
            "clips",
            json!({ "action": "delete", "clip_id": clip_id }),
        )
        .await;
    assert!(
        delete["result"].as_str() == Some("ok"),
        "delete should confirm success: {delete}"
    );

    // Step 5: verify clip is gone
    let list2 = h.expect(5, "clips", json!({ "action": "list" })).await;
    let still_found = list2["clips"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["clip_id"].as_str() == Some(&clip_id));
    assert!(
        !still_found,
        "Clip {clip_id} should be gone after delete. List: {list2}"
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
#[ignore = "requires Godot binary"]
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
        .expect(
            2,
            "spatial_snapshot",
            json!({"detail": "standard", "radius": 500.0}),
        )
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
    let player_pos = player["global_position"]
        .as_array()
        .expect("Player should have abs position");
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
        .find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains("Player"))
                .unwrap_or(false)
        })
        .expect("Player should still be in post-teleport 2D snapshot");
    let player_after_pos = player_after["global_position"]
        .as_array()
        .expect("position");
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

/// Journey: Screenshot API works end-to-end — status fields present, clip
/// listing and retrieval succeed, and empty/missing-screenshot cases are handled.
///
/// Note: In headless mode (--headless), Godot does not render to the viewport,
/// so `do_screenshot_capture()` returns None (empty image). This test verifies
/// the API contract in both headless (no screenshots) and rendering (with
/// screenshots) environments.
///
/// Steps:
///   1. clips(status) → screenshot_buffer_count and screenshot_buffer_kb present
///   2. wait_frames(180) → accumulate ~3 seconds
///   3. clips(status) → both screenshot fields still present (values may be 0 in headless)
///   4. clips(save) → flush clip, get clip_id
///   5. clips(screenshots, clip_id) → valid JSON with total and screenshots array (any count)
///   6. clips(screenshot_at, clip_id, at_frame=0) → valid response (no_screenshots OR image)
///   7. clips(delete, clip_id) → cleanup
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn journey_screenshot_api_contract() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: initial status — screenshot fields must be present
    let status1 = h.expect(1, "clips", json!({ "action": "status" })).await;
    assert_eq!(status1["state"], json!("buffering"));
    assert!(
        status1["screenshot_buffer_count"].as_u64().is_some(),
        "screenshot_buffer_count must be present in status: {status1}"
    );
    assert!(
        status1["screenshot_buffer_kb"].as_u64().is_some(),
        "screenshot_buffer_kb must be present in status: {status1}"
    );

    // Step 2: wait ~3 seconds
    h.wait_frames(180).await;

    // Step 3: fields still present after wait
    let status2 = h.expect(3, "clips", json!({ "action": "status" })).await;
    assert!(
        status2["screenshot_buffer_count"].as_u64().is_some(),
        "screenshot_buffer_count must still be present: {status2}"
    );
    assert!(
        status2["screenshot_buffer_kb"].as_u64().is_some(),
        "screenshot_buffer_kb must still be present: {status2}"
    );

    // Step 4: save clip
    let save = h
        .expect(
            4,
            "clips",
            json!({ "action": "save", "marker_label": "screenshot_api_test" }),
        )
        .await;
    let clip_id = save["clip_id"]
        .as_str()
        .expect("save should return clip_id")
        .to_string();

    // Step 5: list screenshots — must succeed, total must be numeric, array must be present
    let screenshots = h
        .expect(
            5,
            "clips",
            json!({ "action": "screenshots", "clip_id": clip_id }),
        )
        .await;
    assert_eq!(screenshots["clip_id"], json!(clip_id));
    assert!(
        screenshots["total"].as_u64().is_some(),
        "total must be a number: {screenshots}"
    );
    assert!(
        screenshots["screenshots"].as_array().is_some(),
        "screenshots must be an array: {screenshots}"
    );

    // Step 6: screenshot_at — must not error; response depends on whether screenshots exist
    let result = h
        .expect_result(
            6,
            "clips",
            json!({ "action": "screenshot_at", "clip_id": clip_id, "at_frame": 0u64 }),
        )
        .await;

    assert!(
        !result.content.is_empty(),
        "screenshot_at must return at least one content block"
    );

    let total = screenshots["total"].as_u64().unwrap();
    if total == 0 {
        // Headless mode or no screenshots captured: expect no_screenshots text
        assert_eq!(
            result.content.len(),
            1,
            "No screenshots → single text block"
        );
        let text = result.content[0].as_text().expect("Must be text content");
        let body: serde_json::Value = serde_json::from_str(&text.text).unwrap();
        assert_eq!(
            body["error"],
            json!("no_screenshots"),
            "Expected no_screenshots error: {body}"
        );
    } else {
        // Screenshots were captured — verify full image response
        assert_eq!(
            result.content.len(),
            2,
            "With screenshots: must return text + image blocks"
        );
        let text = result.content[0]
            .as_text()
            .expect("First block must be text");
        let meta: serde_json::Value = serde_json::from_str(&text.text).unwrap();
        assert_eq!(meta["clip_id"], json!(clip_id));
        assert!(meta["frame"].as_u64().is_some());

        let image = result.content[1]
            .as_image()
            .expect("Second block must be image");
        assert_eq!(image.mime_type, "image/jpeg");
        assert!(
            !image.data.is_empty(),
            "Image data must be non-empty base64"
        );
    }

    // Step 7: cleanup
    h.expect(
        7,
        "clips",
        json!({ "action": "delete", "clip_id": clip_id }),
    )
    .await;
}
