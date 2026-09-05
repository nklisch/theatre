mod support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use serde_json::json;
use stage_server::server::StageServer;
use support::harness::TestHarness;
use support::mock_addon::QueryHandler;

#[tokio::test]
async fn tool_routes_validated_sequence_to_the_engine() {
    let handler: QueryHandler = Arc::new(|method, params| {
        assert_eq!(method, "execute_action");
        assert_eq!(params["action"], "interaction_sequence");
        assert_eq!(params["steps"][0]["press"][0]["action_name"], "test_jump");
        Ok(json!({
            "action": "interaction_sequence",
            "result": "ok",
            "details": {"steps_completed": 2, "frames_advanced": 3, "new_frame": 42},
            "frame": 42
        }))
    });
    let harness = TestHarness::new(handler).await;

    let result = harness
        .call_tool(
            "spatial_action",
            json!({
                "action": "interaction_sequence",
                "steps": [
                    {"press": [{"action_name": "test_jump"}], "frames": 2},
                    {"release": ["test_jump"], "frames": 1}
                ]
            }),
        )
        .await
        .unwrap();

    assert_eq!(result["details"]["steps_completed"], 2);
    assert_eq!(result["details"]["frames_advanced"], 3);
}

#[tokio::test]
async fn invalid_later_step_is_rejected_before_engine_dispatch() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let handler: QueryHandler = Arc::new(move |_, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(json!({}))
    });
    let harness = TestHarness::new(handler).await;

    let error = harness
        .call_tool(
            "spatial_action",
            json!({
                "action": "interaction_sequence",
                "steps": [
                    {"press": [{"action_name": "test_jump"}], "frames": 1},
                    {"release": ["test_jump"], "frames": 0}
                ]
            }),
        )
        .await
        .unwrap_err();

    assert!(error.message.contains("step 1"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn interaction_sequence_is_present_in_spatial_action_schema() {
    let router = StageServer::router_with_schemas();
    let input = &router.map["spatial_action"].attr.input_schema;
    assert!(input["properties"]["steps"].is_object());
    assert!(
        serde_json::Value::Object((**input).clone())
            .to_string()
            .contains("interaction_sequence")
    );
}

#[tokio::test]
async fn sequence_fields_cannot_be_silently_dropped_before_dispatch() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let handler: QueryHandler = Arc::new(move |_, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(json!({}))
    });
    let harness = TestHarness::new(handler).await;
    let error = harness
        .call_tool(
            "spatial_action",
            json!({
                "action": "pause", "paused": true, "steps": [{"frames": 0}]
            }),
        )
        .await
        .unwrap_err();
    assert!(error.message.contains("'steps'"));

    for (field, value) in [
        ("node", json!("Player")),
        ("paused", json!(true)),
        ("frames", json!(10)),
        ("seconds", json!(1.0)),
        ("position", json!([1, 2])),
        ("rotation_deg", json!(90)),
        ("property", json!("visible")),
        ("value", json!(false)),
        ("signal", json!("ready")),
        ("args", json!([])),
        ("method", json!("hide")),
        ("scene_path", json!("res://a.tscn")),
        ("parent", json!("World")),
        ("name", json!("Child")),
        ("input_action", json!("jump")),
        ("strength", json!(0.5)),
        ("keycode", json!("SPACE")),
        ("pressed", json!(false)),
        ("echo", json!(false)),
        ("button", json!("left")),
    ] {
        let mut params = json!({"action": "interaction_sequence", "steps": [{"frames": 1}]});
        params[field] = value;
        let error = harness
            .call_tool("spatial_action", params)
            .await
            .unwrap_err();
        assert!(error.message.contains(&format!("'{field}'")), "{error:?}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
