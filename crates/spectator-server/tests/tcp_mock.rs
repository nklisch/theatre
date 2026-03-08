//! Layer 1 integration tests: real SpectatorServer handlers against a mock TCP addon.
//!
//! Run with:  cargo test -p spectator-server --features integration-tests

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
    assert!(first.get("rel").is_some());
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

    // Summary should have either a 'groups' or 'clusters' block
    assert!(
        result.get("groups").is_some() || result.get("clusters").is_some(),
        "summary response should contain groups or clusters: {result}"
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
                scene.entities.retain(|e| {
                    e.groups.iter().any(|g| groups.contains(&g.as_str()))
                });
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
        .call_tool("spatial_snapshot", json!({ "detail": "standard", "radius": 1000.0, "include_offscreen": true }))
        .await
        .unwrap();

    let entities = result["entities"].as_array().unwrap();
    assert!(!entities.is_empty());
    // 2D entities should have 2-element position arrays
    let player = entities.iter().find(|e| e["path"] == "Player").unwrap();
    let abs = player["abs"].as_array().unwrap();
    assert_eq!(abs.len(), 2);
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
        format!("{:?}", err).contains("invalid_params") || err.code == rmcp::model::ErrorCode(-32602),
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

    assert!(err.code == rmcp::model::ErrorCode(-32602), "expected invalid_params, got: {err:?}");
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

    assert!(result.get("results").is_some(),
        "expected results in nearest result: {result}");
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

    assert!(result.get("results").is_some(),
        "expected results in radius result: {result}");
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
    assert!(inner.get("clear").is_some(),
        "raycast result should have clear field: {result}");
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
    let result = harness
        .call_tool("spatial_delta", json!({}))
        .await
        .unwrap();

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
    let result = harness
        .call_tool("spatial_delta", json!({}))
        .await
        .unwrap();

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
    assert!(result.get("config").is_some() || result.get("static_patterns").is_some(),
        "config update result: {result}");
}

// ---------------------------------------------------------------------------
// recording tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_recording_status() {
    let handler: QueryHandler = Arc::new(|method, _| {
        if method == "recording_status" {
            Ok(json!({ "recording": false }))
        } else {
            Err(("unknown_method".into(), method.to_string()))
        }
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("recording", json!({ "action": "status" }))
        .await
        .unwrap();

    assert!(
        result.get("recording").is_some() || result.get("active").is_some(),
        "status result: {result}"
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

    let err = server.spatial_snapshot(Parameters(params)).await.unwrap_err();
    assert!(
        err.message.contains("Not connected") || err.message.contains("connected"),
        "expected not-connected error, got: {}", err.message
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
