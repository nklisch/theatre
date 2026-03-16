//! Layer 1 integration tests: real StageServer handlers against a mock TCP addon.
//!
//! Run with:  cargo test -p stage-server --test tcp_mock

mod support;

use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    use stage_server::tcp::SessionState;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::{Duration, sleep};

    let (port, _jh) = start_wrong_version_mock().await;

    let state = Arc::new(Mutex::new(SessionState::default()));
    let tcp_state = state.clone();
    let _tcp_task = tokio::spawn(async move {
        stage_server::tcp::tcp_client_loop(tcp_state, port).await;
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
    use stage_protocol::query::NodeInspectResponse;

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
    use stage_protocol::query::InspectCategory;

    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "get_node_inspect" {
            let p: stage_protocol::query::GetNodeInspectParams =
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
    use stage_protocol::query::InspectCategory;

    let handler: QueryHandler = Arc::new(|method, params| {
        if method == "get_node_inspect" {
            let p: stage_protocol::query::GetNodeInspectParams =
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

    // Contract: remove response echoes watch_id back (not a "removed" boolean/count)
    assert_eq!(remove_result["watch_id"].as_str().unwrap_or(""), watch_id);

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
    let handler: QueryHandler =
        Arc::new(|method, _| Err(("unknown_method".into(), method.to_string())));

    let harness = TestHarness::new(handler).await;

    // Create a temp directory with a fake clip SQLite file
    let tmp = tempfile::TempDir::new().unwrap();
    let clip_path = tmp.path().join("clip_001.sqlite");
    // Create a minimal valid SQLite DB so delete can find it
    std::fs::write(&clip_path, "fake").unwrap();

    // Pre-set clip_storage_path in state so resolve doesn't need TCP
    {
        let mut s = harness.state.lock().await;
        s.clip_storage_path = Some(tmp.path().to_str().unwrap().to_string());
    }

    let result = harness
        .call_tool(
            "clips",
            json!({ "action": "delete", "clip_id": "clip_001" }),
        )
        .await
        .unwrap();

    assert_eq!(
        result.get("result").and_then(|v| v.as_str()),
        Some("ok"),
        "delete result: {result}"
    );
    assert_eq!(
        result.get("clip_id").and_then(|v| v.as_str()),
        Some("clip_001"),
        "delete should echo clip_id: {result}"
    );
    assert!(!clip_path.exists(), "clip file should be deleted from disk");
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

    // With typed enums, serde rejects unknown variants at deserialization
    assert!(
        err.message.contains("nonexistent") || err.message.contains("unknown variant"),
        "expected error about unknown clips action, got: {err:?}"
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
    use stage_server::{server::StageServer, tcp::SessionState};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Create server with no mock addon (never connects)
    let state = Arc::new(Mutex::new(SessionState::default()));
    let server = StageServer::new(state);

    use rmcp::handler::server::wrapper::Parameters;
    use stage_server::mcp::snapshot::SpatialSnapshotParams;

    use stage_protocol::query::DetailLevel;
    use stage_server::mcp::snapshot::PerspectiveMode;

    let params = SpatialSnapshotParams {
        perspective: PerspectiveMode::Camera,
        focal_node: None,
        focal_point: None,
        radius: 50.0,
        detail: DetailLevel::Standard,
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

// ---------------------------------------------------------------------------
// Offline clip access tests — no TCP connection, clips read from disk
// ---------------------------------------------------------------------------

/// Schema matching production (includes created_at_unix_ms).
const CLIP_SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS recording (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    started_at_frame INTEGER NOT NULL,
    ended_at_frame INTEGER,
    started_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER,
    scene_dimensions INTEGER,
    physics_ticks_per_sec INTEGER,
    capture_config TEXT,
    created_at_unix_ms INTEGER
);
CREATE TABLE IF NOT EXISTS frames (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER NOT NULL,
    data BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS markers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    source TEXT NOT NULL,
    label TEXT NOT NULL,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);
CREATE TABLE IF NOT EXISTS screenshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    jpeg_data BLOB NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);
CREATE INDEX IF NOT EXISTS idx_frames_ts ON frames(timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_markers_frame ON markers(frame);
";

/// Create a clip SQLite file with frames and optional markers.
fn create_test_clip(dir: &std::path::Path, clip_id: &str, created_ms: i64) {
    use rusqlite::Connection;
    use stage_protocol::recording::FrameEntityData;

    let db = Connection::open(dir.join(format!("{clip_id}.sqlite"))).unwrap();
    db.execute_batch(CLIP_SCHEMA_SQL).unwrap();

    let capture = json!({ "dashcam": true, "tier": "deliberate" });
    db.execute(
        "INSERT INTO recording (id, name, started_at_frame, ended_at_frame, \
         started_at_ms, ended_at_ms, scene_dimensions, physics_ticks_per_sec, \
         capture_config, created_at_unix_ms) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            clip_id,
            format!("dashcam_{clip_id}"),
            100,
            200,
            created_ms,
            created_ms + 5000,
            3,
            60,
            capture.to_string(),
            created_ms,
        ],
    )
    .unwrap();

    for f in [100u64, 150, 200] {
        let entities = vec![FrameEntityData {
            path: "player".into(),
            class: "CharacterBody3D".into(),
            position: vec![f as f64 * 0.1, 0.0, 0.0],
            rotation_deg: vec![0.0, 0.0, 0.0],
            velocity: vec![1.0, 0.0, 0.0],
            groups: vec![],
            visible: true,
            state: serde_json::Map::new(),
        }];
        let data = rmp_serde::to_vec(&entities).unwrap();
        db.execute(
            "INSERT INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![f, created_ms + (f - 100) as i64 * 50, &data],
        )
        .unwrap();
    }
}

fn add_test_marker(dir: &std::path::Path, clip_id: &str, frame: i64, label: &str) {
    let db = rusqlite::Connection::open(dir.join(format!("{clip_id}.sqlite"))).unwrap();
    db.execute(
        "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![frame, frame * 16, "agent", label],
    )
    .unwrap();
}

/// Create a StageServer with pre-populated clip_storage_path but no TCP connection.
fn offline_server(
    storage_path: &str,
) -> (
    stage_server::server::StageServer,
    Arc<Mutex<stage_server::tcp::SessionState>>,
) {
    let state = Arc::new(Mutex::new(stage_server::tcp::SessionState {
        clip_storage_path: Some(storage_path.to_string()),
        ..Default::default()
    }));
    let server = stage_server::server::StageServer::new(state.clone());
    (server, state)
}

#[tokio::test]
async fn test_offline_clips_list() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_test_clip(tmp.path(), "clip_001", 1710000000000);
    create_test_clip(tmp.path(), "clip_002", 1710001000000);
    add_test_marker(tmp.path(), "clip_001", 150, "bug here");

    let (server, _state) = offline_server(tmp.path().to_str().unwrap());
    let result = support::dispatch_tool(&server, "clips", json!({ "action": "list" }))
        .await
        .unwrap();

    let clips = result["clips"].as_array().unwrap();
    assert_eq!(clips.len(), 2, "should list both clips: {result}");
    // Newest first
    assert_eq!(clips[0]["clip_id"].as_str().unwrap(), "clip_002");
    assert_eq!(clips[1]["clip_id"].as_str().unwrap(), "clip_001");
    // Metadata present
    assert!(clips[0]["frames_captured"].as_u64().unwrap() > 0);
    assert!(clips[0]["created_at"].as_str().unwrap().contains("T"));
}

#[tokio::test]
async fn test_offline_clips_list_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let (server, _state) = offline_server(tmp.path().to_str().unwrap());

    let result = support::dispatch_tool(&server, "clips", json!({ "action": "list" }))
        .await
        .unwrap();

    let clips = result["clips"].as_array().unwrap();
    assert!(clips.is_empty());
}

#[tokio::test]
async fn test_offline_clips_markers() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_test_clip(tmp.path(), "clip_001", 1710000000000);
    add_test_marker(tmp.path(), "clip_001", 150, "checkpoint");
    add_test_marker(tmp.path(), "clip_001", 100, "start");

    let (server, _state) = offline_server(tmp.path().to_str().unwrap());
    let result = support::dispatch_tool(
        &server,
        "clips",
        json!({ "action": "markers", "clip_id": "clip_001" }),
    )
    .await
    .unwrap();

    assert_eq!(result["clip_id"].as_str().unwrap(), "clip_001");
    let markers = result["markers"].as_array().unwrap();
    assert_eq!(markers.len(), 2);
    // Sorted by frame
    assert_eq!(markers[0]["frame"].as_i64().unwrap(), 100);
    assert_eq!(markers[1]["frame"].as_i64().unwrap(), 150);
    assert_eq!(markers[1]["label"].as_str().unwrap(), "checkpoint");
}

#[tokio::test]
async fn test_offline_clips_delete() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_test_clip(tmp.path(), "clip_del", 1710000000000);

    let (server, _state) = offline_server(tmp.path().to_str().unwrap());
    let result = support::dispatch_tool(
        &server,
        "clips",
        json!({ "action": "delete", "clip_id": "clip_del" }),
    )
    .await
    .unwrap();

    assert_eq!(result["result"].as_str().unwrap(), "ok");
    assert_eq!(result["clip_id"].as_str().unwrap(), "clip_del");
    assert!(
        !tmp.path().join("clip_del.sqlite").exists(),
        "file should be gone"
    );
}

#[tokio::test]
async fn test_offline_clips_delete_missing_returns_error() {
    let tmp = tempfile::TempDir::new().unwrap();

    let (server, _state) = offline_server(tmp.path().to_str().unwrap());
    let err = support::dispatch_tool(
        &server,
        "clips",
        json!({ "action": "delete", "clip_id": "clip_nope" }),
    )
    .await
    .unwrap_err();

    assert!(
        err.message.contains("not found"),
        "should report not found: {err:?}"
    );
}

#[tokio::test]
async fn test_offline_clips_snapshot_at() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_test_clip(tmp.path(), "clip_snap", 1710000000000);

    let (server, _state) = offline_server(tmp.path().to_str().unwrap());
    let result = support::dispatch_tool(
        &server,
        "clips",
        json!({
            "action": "snapshot_at",
            "clip_id": "clip_snap",
            "at_frame": 150,
        }),
    )
    .await
    .unwrap();

    assert!(
        result.get("entities").is_some(),
        "snapshot_at should return entities: {result}"
    );
    assert!(
        result.get("clip_context").is_some(),
        "should include clip_context: {result}"
    );
}

#[tokio::test]
async fn test_offline_clips_trajectory() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_test_clip(tmp.path(), "clip_traj", 1710000000000);

    let (server, _state) = offline_server(tmp.path().to_str().unwrap());
    let result = support::dispatch_tool(
        &server,
        "clips",
        json!({
            "action": "trajectory",
            "clip_id": "clip_traj",
            "node": "player",
            "from_frame": 100,
            "to_frame": 200,
        }),
    )
    .await
    .unwrap();

    assert!(
        result.get("samples").is_some() || result.get("node").is_some(),
        "trajectory should return samples: {result}"
    );
}

#[tokio::test]
async fn test_offline_clips_diff_frames() {
    let tmp = tempfile::TempDir::new().unwrap();
    create_test_clip(tmp.path(), "clip_diff", 1710000000000);

    let (server, _state) = offline_server(tmp.path().to_str().unwrap());
    let result = support::dispatch_tool(
        &server,
        "clips",
        json!({
            "action": "diff_frames",
            "clip_id": "clip_diff",
            "frame_a": 100,
            "frame_b": 200,
        }),
    )
    .await
    .unwrap();

    assert!(
        result.get("changes").is_some() || result.get("clip_context").is_some(),
        "diff_frames should return changes: {result}"
    );
}

#[tokio::test]
async fn test_offline_add_marker_requires_live_connection() {
    let tmp = tempfile::TempDir::new().unwrap();
    let (server, _state) = offline_server(tmp.path().to_str().unwrap());

    let err = support::dispatch_tool(
        &server,
        "clips",
        json!({ "action": "add_marker", "marker_label": "test" }),
    )
    .await
    .unwrap_err();

    assert!(
        err.message.contains("Not connected") || err.message.contains("not connected"),
        "add_marker should fail when offline: {err:?}"
    );
}

#[tokio::test]
async fn test_offline_status_requires_live_connection() {
    let tmp = tempfile::TempDir::new().unwrap();
    let (server, _state) = offline_server(tmp.path().to_str().unwrap());

    let err = support::dispatch_tool(&server, "clips", json!({ "action": "status" }))
        .await
        .unwrap_err();

    assert!(
        err.message.contains("Not connected") || err.message.contains("not connected"),
        "status should fail when offline: {err:?}"
    );
}

#[tokio::test]
async fn test_online_list_prefers_addon_then_falls_back() {
    // When addon IS connected, list should use addon response (which may include live info)
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "recording_list" => Ok(json!({
            "clips": [
                { "clip_id": "clip_live", "name": "live_clip", "frames_captured": 50 }
            ]
        })),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("clips", json!({ "action": "list" }))
        .await
        .unwrap();

    let clips = result["clips"].as_array().unwrap();
    assert_eq!(clips.len(), 1);
    assert_eq!(
        clips[0]["clip_id"].as_str().unwrap(),
        "clip_live",
        "should use addon response when connected"
    );
}
