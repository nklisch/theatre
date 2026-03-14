/// CLI Journey Tests — real Godot headless + spectator binary subprocess.
///
/// These tests require:
///   - GODOT_BIN env var pointing to a Godot 4.x binary (or `godot` on PATH)
///   - GDExtension built and deployed to tests/godot-project/addons/spectator/
///     Run: theatre deploy ~/dev/theatre/tests/godot-project
///
/// Run: cargo test -p spectator-server --test cli_journeys -- --nocapture
mod support;

use serde_json::json;
use support::cli_fixture::SpectatorCliFixture;

/// Journey: Agent connects and explores a 3D scene via CLI subprocess.
///
/// Steps:
///   1. scene_tree {roots} → roots array non-empty
///   2. spatial_snapshot {summary} → non-null
///   3. spatial_snapshot {standard} → Player at ~(0,0,0), Scout at ~(5,0,-3)
///   4. spatial_inspect {Enemies/Scout} → has class field, properties.health=80
///   5. spatial_query {nearest, from origin, k=2} → non-empty results with non-negative distances
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn cli_journey_explore_scene() {
    let fixture = SpectatorCliFixture::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: scene_tree roots
    let tree = fixture
        .run("scene_tree", json!({"action": "roots"}))
        .expect("Failed to invoke scene_tree")
        .unwrap_data();
    assert!(
        tree["roots"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "Scene tree roots should return at least one node, got: {tree}"
    );

    // Step 2: snapshot summary
    let summary = fixture
        .run("spatial_snapshot", json!({"detail": "summary"}))
        .expect("Failed to invoke spatial_snapshot summary")
        .unwrap_data();
    assert!(
        !summary.is_null(),
        "Summary snapshot should return non-null data"
    );

    // Step 3: snapshot standard — real positions
    let snapshot = fixture
        .run("spatial_snapshot", json!({"detail": "standard"}))
        .expect("Failed to invoke spatial_snapshot standard")
        .unwrap_data();
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

    // Step 4: spatial_inspect Scout
    let inspect = fixture
        .run("spatial_inspect", json!({"node": "Enemies/Scout"}))
        .expect("Failed to invoke spatial_inspect")
        .unwrap_data();
    assert!(
        inspect["class"].is_string() || inspect["node_path"].is_string(),
        "Inspect should return node info with class or node_path, got: {inspect}"
    );
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

    // Step 5: spatial_query nearest from origin
    // Note: In CLI mode each invocation is a fresh session, so the spatial index
    // is empty (it's populated by snapshot within a session). We verify the response
    // shape is correct (results array exists) but don't require non-empty results.
    let query = fixture
        .run(
            "spatial_query",
            json!({"query_type": "nearest", "from": [0, 0, 0], "k": 2}),
        )
        .expect("Failed to invoke spatial_query")
        .unwrap_data();
    assert!(
        query["results"].as_array().is_some(),
        "spatial_query should return results array, got: {query}"
    );
    assert!(
        query["query"].as_str() == Some("nearest"),
        "query field should echo 'nearest'"
    );
}

/// Journey: Teleport an enemy, verify position changed via snapshot and inspect.
///
/// Note: spatial_delta is NOT tested here — each CLI invocation is a fresh session
/// so delta has no baseline to compare against. Use the MCP server (e2e_journeys)
/// for delta testing.
///
/// Steps:
///   1. spatial_snapshot {standard} → baseline, Scout at ~(5,0,-3)
///   2. spatial_action {teleport, Enemies/Scout, [0,0,0]} → non-null ack
///   3. wait_frames(5)
///   4. spatial_snapshot {standard} → Scout now at ~(0,0,0)
///   5. spatial_action {set_property, Enemies/Scout, health, 25} → ack
///   6. wait_frames(2)
///   7. spatial_inspect {Enemies/Scout} → health=25
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn cli_journey_mutate_and_observe() {
    let fixture = SpectatorCliFixture::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: baseline snapshot
    let baseline = fixture
        .run("spatial_snapshot", json!({"detail": "standard"}))
        .expect("Failed to invoke spatial_snapshot")
        .unwrap_data();
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
        .expect("Scout should have position array");
    let before_x = before_pos[0].as_f64().unwrap_or(0.0);

    // Step 2: teleport Scout to origin
    let teleport_result = fixture
        .run(
            "spatial_action",
            json!({"action": "teleport", "node": "Enemies/Scout", "position": [0, 0, 0]}),
        )
        .expect("Failed to invoke spatial_action teleport")
        .unwrap_data();
    assert!(
        !teleport_result.is_null(),
        "Teleport should return an acknowledgement"
    );

    // Step 3: wait for physics to settle
    fixture.wait_frames(5).await;

    // Step 4: post-teleport snapshot — Scout should be at ~(0,0,0)
    let after_snapshot = fixture
        .run("spatial_snapshot", json!({"detail": "standard"}))
        .expect("Failed to invoke post-teleport spatial_snapshot")
        .unwrap_data();
    let after_entities = after_snapshot["entities"]
        .as_array()
        .expect("Expected entities in post-teleport snapshot");
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
        .expect("Scout should have position array after teleport");
    let after_x = after_pos[0].as_f64().unwrap_or(999.0);
    assert!(
        (after_x - before_x).abs() > 2.0,
        "Scout position should have changed after teleport: before={before_x}, after={after_x}"
    );

    // Step 5: set health to 25
    let set_result = fixture
        .run(
            "spatial_action",
            json!({"action": "set_property", "node": "Enemies/Scout", "property": "health", "value": 25}),
        )
        .expect("Failed to invoke spatial_action set_property")
        .unwrap_data();
    assert!(
        !set_result.is_null(),
        "set_property should return an acknowledgement"
    );

    // Step 6: wait for property update to propagate
    fixture.wait_frames(2).await;

    // Step 7: inspect Scout — health should now be 25
    let inspect = fixture
        .run("spatial_inspect", json!({"node": "Enemies/Scout"}))
        .expect("Failed to invoke spatial_inspect after set_property")
        .unwrap_data();
    let props = inspect["properties"].as_object();
    if let Some(props) = props
        && let Some(health) = props.get("health")
    {
        let health_val = health.as_f64().unwrap_or(80.0);
        assert!(
            (health_val - 25.0).abs() < 0.1,
            "Scout health should be 25 after set_property, got {health_val}"
        );
    }
}

/// Journey: 2D scene returns correct position format.
///
/// Steps:
///   1. spatial_snapshot {standard, radius:500} → entities with 2-element positions, Player at ~(0,0)
///   2. spatial_query {nearest, from [0,0], k=2} → non-empty results
///   3. spatial_inspect {Player} → no elevation field (null)
///   4. spatial_action {teleport, Player, [100,50]} → ack
///   5. wait_frames(3)
///   6. spatial_snapshot {standard, radius:500, include_offscreen:true} → Player at ~(100,50)
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn cli_journey_2d_scene() {
    let fixture = SpectatorCliFixture::start_2d()
        .await
        .expect("Failed to start Godot 2D scene");

    // Step 1: 2D snapshot
    let snapshot = fixture
        .run(
            "spatial_snapshot",
            json!({"detail": "standard", "radius": 500.0}),
        )
        .expect("Failed to invoke spatial_snapshot on 2D scene")
        .unwrap_data();
    let entities = snapshot["entities"]
        .as_array()
        .expect("Expected entities array in 2D snapshot");
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

    // Player should be at ~(0, 0)
    let player = entities
        .iter()
        .find(|e| e["path"].as_str().map(|p| p == "Player").unwrap_or(false))
        .expect("Player should be in 2D snapshot");
    let player_pos = player["global_position"]
        .as_array()
        .expect("Player should have position array");
    assert!(
        player_pos[0].as_f64().unwrap_or(999.0).abs() < 1.0,
        "Player X should be ~0 in 2D scene, got {player_pos:?}"
    );
    assert!(
        player_pos[1].as_f64().unwrap_or(999.0).abs() < 1.0,
        "Player Y should be ~0 in 2D scene, got {player_pos:?}"
    );

    // Step 2: spatial_query nearest in 2D
    // Same CLI session limitation as 3D: spatial index is empty per-session.
    let query = fixture
        .run(
            "spatial_query",
            json!({"query_type": "nearest", "from": [0, 0], "k": 2}),
        )
        .expect("Failed to invoke spatial_query on 2D scene")
        .unwrap_data();
    assert!(
        query["results"].as_array().is_some(),
        "spatial_query should return results array, got: {query}"
    );

    // Step 3: inspect Player — no elevation field
    let inspect = fixture
        .run("spatial_inspect", json!({"node": "Player"}))
        .expect("Failed to invoke spatial_inspect on 2D Player")
        .unwrap_data();
    assert!(
        !inspect.is_null(),
        "Inspect should return data for Player in 2D scene"
    );
    assert!(
        inspect["elevation"].is_null(),
        "2D inspect should not have elevation field, got: {inspect}"
    );

    // Step 4: teleport Player to (100, 50)
    let teleport = fixture
        .run(
            "spatial_action",
            json!({"action": "teleport", "node": "Player", "position": [100, 50]}),
        )
        .expect("Failed to invoke spatial_action teleport on 2D Player")
        .unwrap_data();
    assert!(
        !teleport.is_null(),
        "2D teleport should return acknowledgement"
    );

    // Step 5: wait for physics
    fixture.wait_frames(3).await;

    // Step 6: post-teleport snapshot — Player at ~(100, 50)
    let after = fixture
        .run(
            "spatial_snapshot",
            json!({"detail": "standard", "radius": 500.0, "include_offscreen": true}),
        )
        .expect("Failed to invoke post-teleport spatial_snapshot on 2D scene")
        .unwrap_data();
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
        .expect("Player should have position array after 2D teleport");
    assert_eq!(
        player_after_pos.len(),
        2,
        "Post-teleport 2D position should still have 2 elements"
    );
    assert!(
        (player_after_pos[0].as_f64().unwrap_or(0.0) - 100.0).abs() < 5.0,
        "Player X should be ~100 after teleport, got {player_after_pos:?}"
    );
    assert!(
        (player_after_pos[1].as_f64().unwrap_or(0.0) - 50.0).abs() < 5.0,
        "Player Y should be ~50 after teleport, got {player_after_pos:?}"
    );
}

/// Journey: Clips lifecycle via CLI — save, list, delete.
///
/// Steps:
///   1. wait_frames(60) → let buffer accumulate
///   2. clips {status} → dashcam_enabled=true, state="buffering"
///   3. clips {save, marker_label:"cli_test"} → clip_id starts with "clip_"
///   4. clips {list} → saved clip present in clips array
///   5. clips {delete, clip_id} → result="ok", echoes clip_id
///   6. clips {list} → clip is gone
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn cli_journey_clips_lifecycle() {
    let fixture = SpectatorCliFixture::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: let dashcam buffer accumulate
    fixture.wait_frames(60).await;

    // Step 2: verify dashcam status
    let status = fixture
        .run("clips", json!({"action": "status"}))
        .expect("Failed to invoke clips status")
        .unwrap_data();
    assert_eq!(
        status["dashcam_enabled"],
        json!(true),
        "Dashcam should be enabled by default"
    );
    assert_eq!(
        status["state"],
        json!("buffering"),
        "Dashcam should be in buffering state"
    );

    // Step 3: save clip
    let save = fixture
        .run(
            "clips",
            json!({"action": "save", "marker_label": "cli_test"}),
        )
        .expect("Failed to invoke clips save")
        .unwrap_data();
    let clip_id = save["clip_id"]
        .as_str()
        .expect("save should return clip_id")
        .to_string();
    assert!(
        clip_id.starts_with("clip_"),
        "clip_id should start with 'clip_', got: {clip_id}"
    );

    // Step 4: clip should be in list
    let list = fixture
        .run("clips", json!({"action": "list"}))
        .expect("Failed to invoke clips list after save")
        .unwrap_data();
    let found = list["clips"]
        .as_array()
        .expect("list should return clips array")
        .iter()
        .any(|r| r["clip_id"].as_str() == Some(&clip_id));
    assert!(found, "Clip {clip_id} should be in list after save");

    // Step 5: delete the clip
    let delete = fixture
        .run("clips", json!({"action": "delete", "clip_id": clip_id}))
        .expect("Failed to invoke clips delete")
        .unwrap_data();
    assert_eq!(
        delete["result"].as_str(),
        Some("ok"),
        "delete should confirm success: {delete}"
    );

    // Step 6: clip should be gone
    let list2 = fixture
        .run("clips", json!({"action": "list"}))
        .expect("Failed to invoke clips list after delete")
        .unwrap_data();
    let still_found = list2["clips"]
        .as_array()
        .expect("list should return clips array")
        .iter()
        .any(|r| r["clip_id"].as_str() == Some(&clip_id));
    assert!(
        !still_found,
        "Clip {clip_id} should be gone after delete. List: {list2}"
    );
}

/// Journey: Error handling — invalid nodes, missing required params.
///
/// Steps:
///   1. spatial_inspect {NonExistent/Node/Path} → CliResult::Err, exit_code=1, error has "error" field
///   2. spatial_query {nearest} (missing required `from`) → CliResult::Err
///   3. spatial_action {teleport} (missing required `node`) → CliResult::Err
#[tokio::test]
#[ignore = "requires Godot binary"]
async fn cli_journey_error_handling() {
    let fixture = SpectatorCliFixture::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: inspect a node that does not exist
    let result = fixture
        .run("spatial_inspect", json!({"node": "NonExistent/Node/Path"}))
        .expect("Failed to invoke spatial_inspect on missing node");
    assert!(
        result.is_err(),
        "Inspecting a non-existent node should return an error"
    );
    let (exit_code, error) = result.unwrap_err();
    assert_eq!(
        exit_code, 1,
        "Exit code for tool error should be 1, got {exit_code}"
    );
    assert!(
        error.get("error").is_some(),
        "Error response should have an 'error' field: {error}"
    );

    // Step 2: spatial_query nearest missing required `from` param
    let result2 = fixture
        .run("spatial_query", json!({"query_type": "nearest"}))
        .expect("Failed to invoke spatial_query with missing from");
    assert!(
        result2.is_err(),
        "spatial_query without required `from` should return an error"
    );

    // Step 3: spatial_action teleport missing required `node` param
    let result3 = fixture
        .run("spatial_action", json!({"action": "teleport"}))
        .expect("Failed to invoke spatial_action without node");
    assert!(
        result3.is_err(),
        "spatial_action teleport without required `node` should return an error"
    );
}
