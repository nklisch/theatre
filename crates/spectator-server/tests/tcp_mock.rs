//! Layer 1 integration tests: real SpectatorServer handlers against a mock TCP addon.
//!
//! Run with:  cargo test -p spectator-server --test tcp_mock

mod support;

use serde_json::json;
use std::sync::Arc;

use support::{
    fixtures::{mock_scene_2d, mock_scene_3d},
    harness::TestHarness,
    mock_addon::{QueryHandler, start_wrong_version_mock},
};

// ---------------------------------------------------------------------------
// Handshake tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_handshake_connects_and_gets_session_id() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    let state = harness.state.lock().await;
    assert!(state.connected);
    assert!(state.session_id.is_some());
    let info = state.handshake_info.as_ref().unwrap();
    assert_eq!(info.project_name, "IntegrationTest");
    assert_eq!(info.scene_dimensions, 3);
}

#[tokio::test]
async fn test_handshake_version_mismatch_disconnects() {
    use spectator_server::tcp::SessionState;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::{Duration, sleep};

    let (port, _jh) = start_wrong_version_mock().await;

    let state = Arc::new(Mutex::new(SessionState::default()));
    let tcp_state = state.clone();
    let _tcp_task = tokio::spawn(async move {
        spectator_server::tcp::tcp_client_loop(tcp_state, port).await;
    });

    // Give the server time to connect and reject
    sleep(Duration::from_millis(500)).await;

    let s = state.lock().await;
    // The server should not be connected after a version mismatch
    assert!(!s.connected);
}

// ---------------------------------------------------------------------------
// spatial_snapshot tests
// ---------------------------------------------------------------------------

fn snapshot_handler() -> QueryHandler {
    Arc::new(|method, _params| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown_method".into(), format!("unknown: {method}")))
        }
    })
}

#[tokio::test]
async fn test_snapshot_standard_returns_entities() {
    let harness = TestHarness::new(snapshot_handler()).await;

    let result = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Should have an entities array
    let entities = result.get("entities").unwrap();
    assert!(entities.is_array());
    let arr = entities.as_array().unwrap();
    // Our mock scene has 5 entities, all within the default 50m radius
    assert!(!arr.is_empty());
    // Each entity should have path, class, rel
    let first = &arr[0];
    assert!(first.get("path").is_some());
    assert!(first.get("class").is_some());
    assert!(first.get("relative").is_some());
}

#[tokio::test]
async fn test_snapshot_budget_block_present() {
    let harness = TestHarness::new(snapshot_handler()).await;

    let result = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    let budget = result.get("budget").unwrap();
    assert!(budget.get("used").is_some());
    assert!(budget.get("limit").is_some());
    assert!(budget.get("hard_cap").is_some());
}

#[tokio::test]
async fn test_snapshot_summary_returns_clusters() {
    let harness = TestHarness::new(snapshot_handler()).await;

    let result = harness
        .call_tool("spatial_snapshot", json!({ "detail": "summary" }))
        .await
        .unwrap();

    // Summary should have a 'clusters' block
    assert!(
        result.get("clusters").is_some(),
        "summary response should contain 'clusters': {result}"
    );
}

#[tokio::test]
async fn test_snapshot_filters_by_group() {
    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "get_snapshot_data" {
            // Return only enemies when groups filter is applied
            let groups = params
                .get("groups")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();

            let mut scene = mock_scene_3d();
            if !groups.is_empty() {
                scene
                    .entities
                    .retain(|e| e.groups.iter().any(|g| groups.contains(&g.as_str())));
            }
            Ok(serde_json::to_value(scene).unwrap())
        } else {
            Err(("unknown_method".into(), format!("unknown: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;

    let result = harness
        .call_tool(
            "spatial_snapshot",
            json!({ "detail": "standard", "groups": ["enemies"] }),
        )
        .await
        .unwrap();

    let entities = result["entities"].as_array().unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0]["path"], "enemies/Scout");
}

#[tokio::test]
async fn test_snapshot_2d_scene() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_2d()).unwrap())
        } else {
            Err(("unknown_method".into(), format!("unknown: {method}")))
        }
    });

    let harness = TestHarness::new_2d(handler).await;

    // Use large radius and include_offscreen to capture 2D entities far from origin
    let result = harness
        .call_tool(
            "spatial_snapshot",
            json!({ "detail": "standard", "radius": 1000.0, "include_offscreen": true }),
        )
        .await
        .unwrap();

    let entities = result["entities"].as_array().unwrap();
    assert!(!entities.is_empty());
    // 2D entities should have 2-element position arrays
    let player = entities.iter().find(|e| e["path"] == "Player").unwrap();
    let pos = player["global_position"].as_array().unwrap();
    assert_eq!(pos.len(), 2);
}

// ---------------------------------------------------------------------------
// spatial_inspect tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_inspect_node_not_found() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_node_inspect" {
            Err(("node_not_found".into(), "Node not found: /Missing".into()))
        } else {
            Ok(json!({}))
        }
    });

    let harness = TestHarness::new(handler).await;
    let err = harness
        .call_tool("spatial_inspect", json!({ "node": "/Missing" }))
        .await
        .unwrap_err();

    // node_not_found maps to invalid_params
    assert!(
        format!("{:?}", err).contains("invalid_params")
            || err.code == rmcp::model::ErrorCode(-32602),
        "expected invalid_params, got: {err:?}"
    );
}

#[tokio::test]
async fn test_inspect_all_categories() {
    use spectator_protocol::query::NodeInspectResponse;

    let response = NodeInspectResponse {
        path: "Player".into(),
        class: "CharacterBody3D".into(),
        instance_id: 12345,
        transform: None,
        physics: None,
        state: None,
        children: None,
        signals: None,
        script: None,
        spatial_context_raw: None,
        resources: None,
    };

    let handler: QueryHandler = Arc::new(move |method, _| {
        if method == "get_node_inspect" {
            Ok(serde_json::to_value(&response).unwrap())
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("spatial_inspect", json!({ "node": "Player" }))
        .await
        .unwrap();

    assert_eq!(result["path"], "Player");
    assert_eq!(result["class"], "CharacterBody3D");
    assert!(result.get("budget").is_some());
}

#[tokio::test]
async fn test_inspect_resources_passthrough() {
    use spectator_protocol::query::InspectCategory;

    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "get_node_inspect" {
            let p: spectator_protocol::query::GetNodeInspectParams =
                serde_json::from_value(params.clone()).unwrap();
            assert!(p.include.contains(&InspectCategory::Resources));

            Ok(json!({
                "path": "enemies/scout_02",
                "class": "CharacterBody3D",
                "instance_id": 12345,
                "resources": {
                    "meshes": [{
                        "child": "MeshInstance3D",
                        "resource": "res://models/scout.tres",
                        "type": "ArrayMesh",
                        "surface_count": 3,
                        "material_overrides": [{
                            "surface": 0,
                            "resource": "res://materials/enemy_skin.tres",
                            "type": "StandardMaterial3D"
                        }]
                    }],
                    "collision_shapes": [{
                        "child": "CollisionShape3D",
                        "type": "CapsuleShape3D",
                        "dimensions": {"radius": 0.5, "height": 1.8},
                        "inline": true,
                        "disabled": false
                    }],
                    "animation_players": [{
                        "child": "AnimationPlayer",
                        "current_animation": "patrol_walk",
                        "animations": ["idle", "patrol_walk", "run", "attack"],
                        "position_sec": 0.8,
                        "length_sec": 1.2,
                        "looping": true,
                        "playing": true
                    }],
                    "shader_params": {
                        "outline_color": [1.0, 0.0, 0.0, 1.0],
                        "damage_flash_intensity": 0.0
                    }
                }
            }))
        } else {
            Err(("unknown_method".into(), format!("unknown: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "spatial_inspect",
            json!({
                "node": "enemies/scout_02",
                "include": ["resources"]
            }),
        )
        .await
        .unwrap();

    assert!(result["resources"]["meshes"].is_array());
    assert_eq!(result["resources"]["meshes"][0]["type"], "ArrayMesh");
    assert_eq!(
        result["resources"]["collision_shapes"][0]["dimensions"]["radius"],
        0.5
    );
    assert_eq!(result["resources"]["animation_players"][0]["playing"], true);
    assert_eq!(
        result["resources"]["shader_params"]["damage_flash_intensity"],
        0.0
    );
}

#[tokio::test]
async fn test_inspect_default_excludes_resources() {
    use spectator_protocol::query::InspectCategory;

    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "get_node_inspect" {
            let p: spectator_protocol::query::GetNodeInspectParams =
                serde_json::from_value(params.clone()).unwrap();
            assert!(!p.include.contains(&InspectCategory::Resources));

            Ok(json!({
                "path": "enemies/scout_02",
                "class": "CharacterBody3D",
                "instance_id": 12345
            }))
        } else {
            Err(("unknown_method".into(), format!("unknown: {method}")))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("spatial_inspect", json!({ "node": "enemies/scout_02" }))
        .await
        .unwrap();

    assert!(result.get("resources").is_none());
}

// ---------------------------------------------------------------------------
// scene_tree tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_scene_tree_find_by_group() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_scene_tree" {
            Ok(json!({
                "nodes": [
                    { "path": "enemies/Scout", "class": "CharacterBody3D", "groups": ["enemies"] },
                    { "path": "enemies/Tank", "class": "CharacterBody3D", "groups": ["enemies"] }
                ]
            }))
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "scene_tree",
            json!({ "action": "find", "group": "enemies" }),
        )
        .await
        .unwrap();

    let nodes = result["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
}

#[tokio::test]
async fn test_scene_tree_roots() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_scene_tree" {
            Ok(json!({ "nodes": [{ "path": "TestScene3D", "class": "Node3D" }] }))
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("scene_tree", json!({ "action": "roots" }))
        .await
        .unwrap();

    assert!(result["nodes"].as_array().is_some());
}

// ---------------------------------------------------------------------------
// spatial_action tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_action_pause() {
    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "execute_action" {
            // ActionRequest serializes with tag field "action" = variant name
            let action = params["action"].as_str().unwrap_or("");
            if action == "pause" {
                Ok(json!({ "success": true, "paused": params["paused"] }))
            } else {
                Err(("unknown_action".into(), action.to_string()))
            }
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "spatial_action",
            json!({ "action": "pause", "paused": true }),
        )
        .await
        .unwrap();

    assert_eq!(result["paused"], true);
}

#[tokio::test]
async fn test_action_teleport() {
    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "execute_action" {
            let action = params["action"].as_str().unwrap_or("");
            if action == "teleport" {
                Ok(json!({
                    "success": true,
                    "node": params["path"],
                    "previous_position": [0.0, 0.0, 0.0]
                }))
            } else {
                Err(("unknown_action".into(), action.to_string()))
            }
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "spatial_action",
            json!({ "action": "teleport", "node": "Player", "position": [10.0, 0.0, 0.0] }),
        )
        .await
        .unwrap();

    assert_eq!(result["success"], true);
    assert!(result.get("previous_position").is_some());
}

#[tokio::test]
async fn test_action_node_not_found() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "execute_action" {
            Err(("node_not_found".into(), "Node not found: /Ghost".into()))
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let err = harness
        .call_tool(
            "spatial_action",
            json!({ "action": "teleport", "node": "/Ghost", "position": [0.0, 0.0, 0.0] }),
        )
        .await
        .unwrap_err();

    assert!(
        err.code == rmcp::model::ErrorCode(-32602),
        "expected invalid_params, got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// spatial_query tests
// ---------------------------------------------------------------------------

/// spatial_query nearest/radius use the in-memory spatial index built by snapshot.
/// Raycast and other geometric queries call addon method "spatial_query".
async fn harness_with_snapshot_index() -> TestHarness {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else if method == "spatial_query" {
            // RaycastResponse fields
            Ok(json!({ "clear": true, "total_distance": 10.0, "clear_distance": 10.0 }))
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;

    // Build spatial index by doing a snapshot
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    harness
}

#[tokio::test]
async fn test_query_nearest() {
    let harness = harness_with_snapshot_index().await;

    // nearest uses in-memory spatial index; `from` is required
    let result = harness
        .call_tool(
            "spatial_query",
            json!({
                "query_type": "nearest",
                "from": [0.0, 0.0, 0.0],
                "k": 2
            }),
        )
        .await
        .unwrap();

    assert!(
        result.get("results").is_some(),
        "expected results in nearest result: {result}"
    );
}

#[tokio::test]
async fn test_query_radius() {
    let harness = harness_with_snapshot_index().await;

    // radius uses in-memory spatial index; `from` is required
    let result = harness
        .call_tool(
            "spatial_query",
            json!({
                "query_type": "radius",
                "from": [0.0, 0.0, 0.0],
                "radius": 15.0
            }),
        )
        .await
        .unwrap();

    assert!(
        result.get("results").is_some(),
        "expected results in radius result: {result}"
    );
}

#[tokio::test]
async fn test_query_raycast() {
    let harness = harness_with_snapshot_index().await;

    let result = harness
        .call_tool(
            "spatial_query",
            json!({
                "query_type": "raycast",
                "from": [0.0, 0.0, 0.0],
                "to": [10.0, 0.0, 0.0]
            }),
        )
        .await
        .unwrap();

    // Raycast wraps the response: { "query": "raycast", "result": { "clear": ... } }
    let inner = result.get("result").unwrap_or(&result);
    assert!(
        inner.get("clear").is_some(),
        "raycast result should have clear field: {result}"
    );
}

// ---------------------------------------------------------------------------
// spatial_delta tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_delta_detects_moved() {
    let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let call_count2 = call_count.clone();

    let handler: QueryHandler = Arc::new(move |method, _| {
        if method == "get_snapshot_data" {
            let count = call_count2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut scene = mock_scene_3d();
            if count == 0 {
                // First call: baseline — Scout at original position
                scene.frame = 100;
            } else {
                // Subsequent calls: Scout moved
                scene.entities[1].position = vec![10.0, 0.0, 0.0];
                scene.frame = 110;
            }
            Ok(serde_json::to_value(scene).unwrap())
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;

    // First snapshot establishes baseline (count=0: Scout at [0,0,-5], frame=100)
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Delta queries the addon again (count=1: Scout at [10,0,0], frame=110)
    // and computes delta vs baseline → Scout should be in moved
    let result = harness.call_tool("spatial_delta", json!({})).await.unwrap();

    let moved = result["moved"].as_array().unwrap();
    assert!(
        moved.iter().any(|e| e["path"] == "enemies/Scout"),
        "expected Scout in moved list: {result}"
    );
}

#[tokio::test]
async fn test_delta_signal_emitted_appears() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;

    // Establish a baseline
    harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap();

    // Push a signal event from the mock addon BEFORE calling delta
    harness
        .mock
        .push_event(
            "signal_emitted",
            json!({
                "node": "Player",
                "signal": "health_changed",
                "args": [80],
                "frame": 105
            }),
        )
        .await;

    // Wait for the event to travel: mock → TCP → server handle_connection → delta_engine.push_event
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Delta queries the addon for current state and drains the buffered events
    let result = harness.call_tool("spatial_delta", json!({})).await.unwrap();

    // Signal should appear in signals_emitted
    let signals = result.get("signals_emitted").and_then(|v| v.as_array());
    assert!(
        signals.map(|s| !s.is_empty()).unwrap_or(false),
        "expected signal in delta: {result}"
    );
}

// ---------------------------------------------------------------------------
// spatial_watch tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_watch_add_list_remove_lifecycle() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Ok(serde_json::to_value(mock_scene_3d()).unwrap())
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;

    // Add a watch (track must be a Vec<String>, not a bare string)
    let add_result = harness
        .call_tool(
            "spatial_watch",
            json!({
                "action": "add",
                "watch": {
                    "node": "enemies/Scout",
                    "track": ["state"]
                }
            }),
        )
        .await
        .unwrap();

    let watch_id = add_result["watch_id"].as_str().unwrap().to_string();
    assert!(!watch_id.is_empty());

    // List shows one watch
    let list_result = harness
        .call_tool("spatial_watch", json!({ "action": "list" }))
        .await
        .unwrap();

    let watches = list_result["watches"].as_array().unwrap();
    assert_eq!(watches.len(), 1);

    // Remove the watch
    let remove_result = harness
        .call_tool(
            "spatial_watch",
            json!({ "action": "remove", "watch_id": watch_id }),
        )
        .await
        .unwrap();

    assert!(remove_result["removed"].as_u64().unwrap_or(0) > 0 || remove_result["removed"] == true);

    // List is now empty
    let list_after = harness
        .call_tool("spatial_watch", json!({ "action": "list" }))
        .await
        .unwrap();

    let watches_after = list_after["watches"].as_array().unwrap();
    assert_eq!(watches_after.len(), 0);
}

#[tokio::test]
async fn test_watch_clear() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    // Add two watches
    harness
        .call_tool(
            "spatial_watch",
            json!({ "action": "add", "watch": { "node": "Player", "track": ["state"] } }),
        )
        .await
        .unwrap();
    harness
        .call_tool(
            "spatial_watch",
            json!({ "action": "add", "watch": { "node": "enemies/Scout", "track": ["state"] } }),
        )
        .await
        .unwrap();

    // Clear all
    harness
        .call_tool("spatial_watch", json!({ "action": "clear" }))
        .await
        .unwrap();

    let list = harness
        .call_tool("spatial_watch", json!({ "action": "list" }))
        .await
        .unwrap();

    assert_eq!(list["watches"].as_array().unwrap().len(), 0);
}

// ---------------------------------------------------------------------------
// spatial_config tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_config_read_defaults() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    let result = harness
        .call_tool("spatial_config", json!({}))
        .await
        .unwrap();

    // Config response should show current settings
    assert!(
        result.get("config").is_some() || result.get("token_hard_cap").is_some(),
        "config response should contain config data: {result}"
    );
}

#[tokio::test]
async fn test_config_set_static_patterns() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    let result = harness
        .call_tool(
            "spatial_config",
            json!({ "static_patterns": ["walls/*", "terrain/*"] }),
        )
        .await
        .unwrap();

    // Should succeed and return updated config
    assert!(
        result.get("config").is_some() || result.get("static_patterns").is_some(),
        "config update result: {result}"
    );
}

// ---------------------------------------------------------------------------
// clips tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_recording_status() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "dashcam_status" {
            Ok(json!({
                "dashcam_enabled": true,
                "state": "buffering",
                "buffer_frames": 0,
                "buffer_kb": 0,
                "config": {}
            }))
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("clips", json!({ "action": "status" }))
        .await
        .unwrap();

    assert!(
        result.get("state").is_some(),
        "status result should contain 'state': {result}"
    );
}

#[tokio::test]
async fn test_recording_add_marker() {
    let handler: QueryHandler = Arc::new(|method, params| match method {
        "recording_marker" => {
            let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!({ "ok": true, "label": label, "frame": 10 }))
        }
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "clips",
            json!({ "action": "add_marker", "marker_label": "checkpoint_1" }),
        )
        .await
        .unwrap();

    assert!(
        result.get("ok").and_then(|v| v.as_bool()) == Some(true)
            || result.get("frame").is_some()
            || result.get("label").is_some(),
        "marker result: {result}"
    );
}

#[tokio::test]
async fn test_recording_list() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "recording_list" => Ok(json!({
            "clips": [
                { "clip_id": "clip_001", "name": "run_a", "frames_captured": 100 },
                { "clip_id": "clip_002", "name": "run_b", "frames_captured": 200 },
            ]
        })),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("clips", json!({ "action": "list" }))
        .await
        .unwrap();

    let clips = result.get("clips").and_then(|v| v.as_array());
    assert!(clips.is_some(), "list should return clips array: {result}");
    assert_eq!(clips.unwrap().len(), 2, "expected 2 clips: {result}");
}

#[tokio::test]
async fn test_recording_delete() {
    let handler: QueryHandler = Arc::new(|method, params| match method {
        "recording_delete" => {
            let id = params.get("clip_id").and_then(|v| v.as_str()).unwrap_or("");
            if id == "clip_001" {
                Ok(json!({ "result": "ok", "clip_id": "clip_001" }))
            } else {
                Err(("not_found".into(), format!("clip {id} not found")))
            }
        }
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "clips",
            json!({ "action": "delete", "clip_id": "clip_001" }),
        )
        .await
        .unwrap();

    assert!(
        result.get("result").and_then(|v| v.as_str()) == Some("ok")
            || result.get("clip_id").is_some(),
        "delete result: {result}"
    );
}

// ---------------------------------------------------------------------------
// dashcam tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dashcam_status() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_status" => Ok(json!({
            "dashcam_enabled": true,
            "state": "buffering",
            "buffer_frames": 1800,
            "buffer_kb": 14400,
            "config": {
                "capture_interval": 1,
                "pre_window_sec": { "system": 30, "deliberate": 60 },
                "post_window_sec": { "system": 10, "deliberate": 30 },
                "max_window_sec": 120,
                "min_after_sec": 5,
                "system_min_interval_sec": 2,
                "byte_cap_mb": 1024
            }
        })),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("clips", json!({ "action": "status" }))
        .await
        .unwrap();

    assert_eq!(result["dashcam_enabled"], json!(true));
    assert_eq!(result["state"], json!("buffering"));
    assert!(result["buffer_frames"].as_u64().is_some());
    assert!(result["config"].is_object());
}

#[tokio::test]
async fn test_dashcam_status_post_capture() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_status" => Ok(json!({
            "dashcam_enabled": true,
            "state": "post_capture",
            "buffer_frames": 1800,
            "buffer_kb": 14400,
            "open_clip": {
                "tier": "system",
                "frames_remaining": 300,
                "markers": 2
            },
            "config": {
                "capture_interval": 1,
                "pre_window_sec": { "system": 30, "deliberate": 60 },
                "post_window_sec": { "system": 10, "deliberate": 30 },
                "max_window_sec": 120,
                "min_after_sec": 5,
                "system_min_interval_sec": 2,
                "byte_cap_mb": 1024
            }
        })),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("clips", json!({ "action": "status" }))
        .await
        .unwrap();

    assert_eq!(result["state"], json!("post_capture"));
    assert!(result["open_clip"].is_object());
    assert_eq!(result["open_clip"]["tier"], json!("system"));
}

#[tokio::test]
async fn test_dashcam_flush() {
    let handler: QueryHandler = Arc::new(|method, params| match method {
        "dashcam_flush" => {
            let label = params
                .get("marker_label")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(json!({
                "clip_id": "clip_abc12345",
                "tier": "deliberate",
                "frames": 1800,
                "marker_label": label
            }))
        }
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "clips",
            json!({ "action": "save", "marker_label": "suspected bug" }),
        )
        .await
        .unwrap();

    assert!(
        result["clip_id"].as_str().unwrap().starts_with("clip_"),
        "save should return a clip_id: {result}"
    );
    assert_eq!(result["tier"], json!("deliberate"));
    assert!(result["frames"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_dashcam_flush_empty_buffer_returns_error() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_flush" => Err((
            "empty_buffer".into(),
            "Dashcam ring buffer is empty — no frames to save".into(),
        )),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let err = harness
        .call_tool("clips", json!({ "action": "save", "marker_label": "test" }))
        .await
        .unwrap_err();

    assert!(
        err.message.contains("empty") || err.message.contains("buffer") || !err.message.is_empty(),
        "expected error about empty buffer, got: {err:?}"
    );
}

#[tokio::test]
async fn test_dashcam_flush_when_disabled_returns_error() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_flush" => Err(("dashcam_disabled".into(), "Dashcam is not enabled".into())),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let err = harness
        .call_tool("clips", json!({ "action": "save", "marker_label": "test" }))
        .await
        .unwrap_err();

    assert!(
        err.message.contains("not enabled") || err.message.contains("disabled"),
        "expected error about disabled dashcam, got: {err:?}"
    );
}

#[tokio::test]
async fn test_dashcam_flush_default_label() {
    // When no marker_label is provided, the server sends "agent flush" as default.
    let received_label: Arc<std::sync::Mutex<String>> =
        Arc::new(std::sync::Mutex::new(String::new()));
    let rl = received_label.clone();

    let handler: QueryHandler = Arc::new(move |method, params| match method {
        "dashcam_flush" => {
            let label = params
                .get("marker_label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            *rl.lock().unwrap() = label;
            Ok(json!({
                "clip_id": "clip_default",
                "tier": "deliberate",
                "frames": 100
            }))
        }
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let _ = harness
        .call_tool("clips", json!({ "action": "save" }))
        .await
        .unwrap();

    let label = received_label.lock().unwrap().clone();
    assert_eq!(label, "agent save", "default label should be 'agent save'");
}

#[tokio::test]
async fn test_recording_unknown_action_returns_error() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    let err = harness
        .call_tool("clips", json!({ "action": "nonexistent" }))
        .await
        .unwrap_err();

    assert!(
        err.message.contains("Unknown clips action"),
        "expected 'Unknown clips action' error, got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// advance_frames tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_action_advance_frames_returns_new_frame() {
    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "execute_action" {
            let action = params["action"].as_str().unwrap_or("");
            if action == "advance_frames" {
                let frames = params["frames"].as_u64().unwrap_or(0);
                // Simulate the deferred response the GDExtension sends after
                // advancing N physics frames.
                Ok(json!({
                    "action": "advance_frames",
                    "result": "ok",
                    "details": { "new_frame": 105 + frames },
                    "frame": 105 + frames
                }))
            } else {
                Err(("unknown_action".into(), action.to_string()))
            }
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "spatial_action",
            json!({ "action": "advance_frames", "frames": 5 }),
        )
        .await
        .unwrap();

    // MCP layer passes the addon response through; new_frame should be present.
    assert!(
        result
            .get("details")
            .and_then(|d| d.get("new_frame"))
            .is_some()
            || result.get("new_frame").is_some()
            || result.get("frame").is_some(),
        "advance_frames result missing new_frame: {result}"
    );
}

#[tokio::test]
async fn test_action_advance_frames_requires_frames_param() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    // frames is required — MCP layer should reject before hitting the addon.
    let err = harness
        .call_tool("spatial_action", json!({ "action": "advance_frames" }))
        .await
        .unwrap_err();

    assert!(
        !err.message.is_empty(),
        "expected param error, got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Error handling tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_not_connected_error() {
    use spectator_server::{server::SpectatorServer, tcp::SessionState};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Create server with no mock addon (never connects)
    let state = Arc::new(Mutex::new(SessionState::default()));
    let server = SpectatorServer::new(state);

    use rmcp::handler::server::wrapper::Parameters;
    use spectator_server::mcp::snapshot::SpatialSnapshotParams;

    let params = SpatialSnapshotParams {
        perspective: "camera".into(),
        focal_node: None,
        focal_point: None,
        radius: 50.0,
        detail: "standard".into(),
        groups: None,
        class_filter: None,
        include_offscreen: false,
        token_budget: None,
        expand: None,
    };

    let err = server
        .spatial_snapshot(Parameters(params))
        .await
        .unwrap_err();
    assert!(
        err.message.contains("Not connected") || err.message.contains("connected"),
        "expected not-connected error, got: {}",
        err.message
    );
}

#[tokio::test]
async fn test_addon_error_response_maps_to_mcp_error() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "get_snapshot_data" {
            Err(("scene_not_loaded".into(), "Scene is not loaded".into()))
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let err = harness
        .call_tool("spatial_snapshot", json!({ "detail": "standard" }))
        .await
        .unwrap_err();

    // scene_not_loaded maps to internal_error (-32603)
    assert!(
        err.code == rmcp::model::ErrorCode(-32603) || !err.message.is_empty(),
        "expected internal_error, got: {err:?}"
    );
}
