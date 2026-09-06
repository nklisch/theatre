//! One-shot session limitations are usage errors, not successful ephemeral writes.
mod support;

use serde_json::{Value, json};
use std::process::{Command, Output};

fn run(tool: &str, params: Value) -> Output {
    // Port zero cannot host a listener. A temporary project avoids local TOML
    // overriding it, making accidental transport access visible and deterministic.
    let project = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_stage"))
        .args([tool, &params.to_string()])
        .env("THEATRE_PROJECT_DIR", project.path())
        .env("THEATRE_PORT", "0")
        .output()
        .unwrap()
}

#[test]
fn stateful_workflows_fail_before_connecting_with_a_persistent_alternative() {
    let calls = [
        ("project_select", json!({"project_path":"/unused"})),
        ("spatial_delta", json!({})),
        (
            "spatial_watch",
            json!({"action":"add", "watch":{"node":"player"}}),
        ),
        (
            "spatial_watch",
            json!({"action":"remove", "watch_id":"watch_1"}),
        ),
        ("spatial_watch", json!({"action":"list"})),
        ("spatial_watch", json!({"action":"clear"})),
        ("spatial_config", json!({"token_hard_cap":3000})),
        ("spatial_config", json!({"expose_internals":false})),
        ("spatial_config", json!({"static_patterns":[]})),
        (
            "spatial_action",
            json!({"action":"pause", "paused":true, "return_delta":true}),
        ),
    ];
    for (tool, params) in calls {
        let output = run(tool, params);
        assert_eq!(output.status.code(), Some(2), "{tool}: {output:?}");
        assert!(output.stderr.is_empty(), "{tool}: {output:?}");
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["error"], "persistent_session_required");
        assert!(error["hint"].as_str().unwrap().contains("stage serve"));
        assert!(error["hint"].as_str().unwrap().contains("same session"));
        if tool == "spatial_action" {
            assert!(
                error["message"]
                    .as_str()
                    .unwrap()
                    .contains("No action was performed")
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires Godot binary and deployed Stage addon"]
async fn separate_cli_snapshot_cannot_supply_a_baseline_and_config_read_still_works() {
    let fixture = support::cli_fixture::StageCliFixture::start_3d()
        .await
        .unwrap();
    let snapshot = fixture
        .run("spatial_snapshot", json!({}))
        .unwrap()
        .unwrap_data();
    assert!(snapshot.is_object());
    let (status, error) = fixture
        .run("spatial_delta", json!({}))
        .unwrap()
        .unwrap_err();
    assert_eq!(status, 2);
    assert_eq!(error["error"], "persistent_session_required");
    let config = fixture
        .run("spatial_config", json!({}))
        .unwrap()
        .unwrap_data();
    assert_eq!(config["result"], "ok");
    assert!(config["config"]["token_hard_cap"].is_number());
}

#[test]
fn useful_one_shot_calls_still_reach_transport() {
    for (tool, params) in [
        ("spatial_snapshot", json!({})),
        ("spatial_inspect", json!({"node":"player"})),
        ("scene_tree", json!({"action":"roots"})),
        (
            "spatial_query",
            json!({"query_type":"nearest", "from":"player"}),
        ),
        ("spatial_config", json!({})),
        ("spatial_config", json!({"token_hard_cap":null})),
        ("spatial_action", json!({"action":"pause", "paused":true})),
        (
            "spatial_action",
            json!({"action":"pause", "paused":true,"return_delta":false}),
        ),
        ("clips", json!({"action":"status"})),
    ] {
        let output = run(tool, params);
        assert_eq!(output.status.code(), Some(1), "{tool}: {output:?}");
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["error"], "connection_failed", "{tool}: {error}");
    }
}

#[test]
fn sequence_field_mismatches_are_usage_errors_before_connection() {
    for params in [
        json!({"action":"pause", "paused":true, "steps":[{"frames":0}]}),
        json!({"action":"interaction_sequence", "steps":[{"frames":1}], "frames":10}),
        json!({"action":"interaction_sequence", "steps":[{"frames":1}], "strength":0.5}),
        json!({"action":"interaction_sequence", "steps":[{"frames":1}], "echo":false}),
    ] {
        let output = run("spatial_action", params);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["error"], "invalid_parameters");
        assert!(!error["message"].as_str().unwrap().contains("connection"));
    }
}
