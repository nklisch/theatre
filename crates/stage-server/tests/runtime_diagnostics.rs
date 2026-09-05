mod support;

use std::sync::Arc;

use serde_json::json;
use stage_server::server::StageServer;
use support::harness::TestHarness;
use support::mock_addon::{QueryHandler, mock_identity};

fn diagnostics_handler() -> QueryHandler {
    Arc::new(|method, _params| {
        assert_eq!(method, "runtime_diagnostics");
        Ok(json!({
            "identity": mock_identity(),
            "available": true,
            "diagnostics": [
                {
                    "sequence": 7,
                    "kind": "warning",
                    "message": "older",
                    "origin": {"function": "old", "file": "res://old.gd", "line": 4},
                    "backtrace": []
                },
                {
                    "sequence": 8,
                    "kind": "script_error",
                    "message": "newer",
                    "origin": {"function": "new", "file": "res://new.gd", "line": 9},
                    "backtrace": [{"function": "new", "file": "res://new.gd", "line": 9}]
                }
            ],
            "retained_count": 2,
            "omitted_count": 5,
            "limits": {
                "queue_capacity": 128,
                "message_max_chars": 2048,
                "file_max_chars": 512,
                "function_max_chars": 256,
                "backtrace_max_frames": 16
            },
            "limitations": ["registration is not historical"]
        }))
    })
}

fn oversized_diagnostic_handler() -> QueryHandler {
    Arc::new(|method, _params| {
        assert_eq!(method, "runtime_diagnostics");
        Ok(json!({
            "identity": mock_identity(),
            "available": true,
            "diagnostics": [{
                "sequence": 99,
                "kind": "error",
                "message": "x".repeat(2048),
                "origin": {"function": "large", "file": "res://large.gd", "line": 1},
                "backtrace": []
            }],
            "retained_count": 1,
            "omitted_count": 0,
            "limits": {
                "queue_capacity": 128,
                "message_max_chars": 2048,
                "file_max_chars": 512,
                "function_max_chars": 256,
                "backtrace_max_frames": 16
            },
            "limitations": []
        }))
    })
}

#[tokio::test]
async fn oversized_newest_entry_reports_actionable_budget_recovery() {
    let harness = TestHarness::new(oversized_diagnostic_handler()).await;
    let error = harness
        .call_tool(
            "runtime_diagnostics",
            json!({"max_entries": 1, "token_budget": 200}),
        )
        .await
        .unwrap_err();
    assert!(error.message.contains("sequence 99"));
    assert!(error.message.contains("token_budget >="));
    assert!(error.message.contains("token_hard_cap"));
    assert!(
        error
            .message
            .contains("retained diagnostics were not modified")
    );

    let recovered = harness
        .call_tool(
            "runtime_diagnostics",
            json!({"max_entries": 1, "token_budget": 5000}),
        )
        .await
        .unwrap();
    assert_eq!(recovered["diagnostics"][0]["sequence"], 99);
    assert_eq!(recovered["returned_count"], 1);
}

#[tokio::test]
async fn tool_returns_newest_first_with_truthful_counts_and_budget() {
    let harness = TestHarness::new(diagnostics_handler()).await;
    let result = harness
        .call_tool("runtime_diagnostics", json!({"max_entries": 1}))
        .await
        .unwrap();

    assert_eq!(result["identity"], json!(mock_identity()));
    assert_eq!(result["retained_count"], 2);
    assert_eq!(result["omitted_count"], 5);
    assert_eq!(result["eligible_count"], 2);
    assert_eq!(result["returned_count"], 1);
    assert_eq!(result["response_omitted_count"], 1);
    assert_eq!(result["diagnostics"][0]["sequence"], 8);
    assert_eq!(result["next_before_sequence"], 8);
    assert!(result["budget"]["used"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn pagination_reads_older_retained_diagnostics() {
    let harness = TestHarness::new(diagnostics_handler()).await;
    let result = harness
        .call_tool(
            "runtime_diagnostics",
            json!({"before_sequence": 8, "max_entries": 20}),
        )
        .await
        .unwrap();

    assert_eq!(result["eligible_count"], 1);
    assert_eq!(result["returned_count"], 1);
    assert_eq!(result["response_omitted_count"], 0);
    assert_eq!(result["diagnostics"][0]["sequence"], 7);
    assert!(result["next_before_sequence"].is_null());
}

#[tokio::test]
async fn session_hard_cap_limits_diagnostic_response() {
    let harness = TestHarness::new(diagnostics_handler()).await;
    harness.state.lock().await.config.token_hard_cap = 200;
    let result = harness
        .call_tool(
            "runtime_diagnostics",
            json!({"max_entries": 20, "token_budget": 5000}),
        )
        .await
        .unwrap();

    assert_eq!(result["budget"]["limit"], 200);
    assert_eq!(result["budget"]["hard_cap"], 200);
    assert!(
        result["budget"]["used"].as_u64().unwrap() <= result["budget"]["limit"].as_u64().unwrap()
    );
    assert_eq!(
        result["response_omitted_count"].as_u64().unwrap()
            + result["returned_count"].as_u64().unwrap(),
        result["eligible_count"].as_u64().unwrap()
    );
}

#[tokio::test]
async fn max_entries_is_validated_before_engine_query() {
    let harness = TestHarness::new(diagnostics_handler()).await;
    let error = harness
        .call_tool("runtime_diagnostics", json!({"max_entries": 129}))
        .await
        .unwrap_err();
    assert!(error.message.contains("between 1 and 128"));
}

#[test]
fn runtime_diagnostics_has_typed_input_and_output_schema() {
    let router = StageServer::router_with_schemas();
    let tool = &router.map["runtime_diagnostics"].attr;
    assert_eq!(
        tool.input_schema["properties"]["max_entries"]["type"],
        "integer"
    );
    let output = tool.output_schema.as_ref().unwrap();
    assert!(output["properties"]["identity"].is_object());
    assert!(output["properties"]["diagnostics"].is_object());
    assert!(output["properties"]["limits"].is_object());
}

#[tokio::test]
async fn empty_and_exhausted_pages_respect_soft_and_hard_budgets() {
    for empty_queue in [true, false] {
        let base = diagnostics_handler();
        let handler: QueryHandler = Arc::new(move |method, params| {
            let mut value = base(method, params)?;
            if empty_queue {
                value["diagnostics"] = json!([]);
                value["retained_count"] = json!(0);
            }
            Ok(value)
        });
        let harness = TestHarness::new(handler).await;
        for hard_limit in [false, true] {
            let original_cap = harness.state.lock().await.config.token_hard_cap;
            if hard_limit {
                harness.state.lock().await.config.token_hard_cap = 1;
            }
            let error = harness
                .call_tool(
                    "runtime_diagnostics",
                    json!({
                        "before_sequence": 1,
                        "token_budget": if hard_limit {5000} else {1}
                    }),
                )
                .await
                .unwrap_err();
            assert!(error.message.contains("token_budget >="));
            assert!(error.message.contains("token_hard_cap"));
            assert!(
                error
                    .message
                    .contains("retained diagnostics were not modified")
            );
            harness.state.lock().await.config.token_hard_cap = original_cap;
        }
        let recovered = harness
            .call_tool("runtime_diagnostics", json!({}))
            .await
            .unwrap();
        assert_eq!(recovered["retained_count"], if empty_queue { 0 } else { 2 });
    }
}
