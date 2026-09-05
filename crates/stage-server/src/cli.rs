use anyhow::Result;
use rmcp::model::ErrorData as McpError;
use serde_json::Value;
use std::io::IsTerminal;
use std::sync::Arc;
use tokio::sync::Mutex;

use stage_server::{config, tcp};

/// All supported tool names, in canonical order.
pub const TOOLS: &[&str] = &[
    "spatial_snapshot",
    "spatial_inspect",
    "scene_tree",
    "spatial_action",
    "spatial_query",
    "spatial_delta",
    "spatial_watch",
    "spatial_config",
    "clips",
    "runtime_status",
    "runtime_diagnostics",
    "viewport",
    "feedback",
];

/// Default TCP port for connecting to the Godot addon.
const DEFAULT_PORT: u16 = 9077;

/// Entry point for CLI one-shot mode.
///
/// `tool` — the tool name to invoke.
/// `json_arg` — optional JSON string from CLI arg; if None and stdin is piped, read from stdin.
pub async fn run(tool: &str, json_arg: Option<&str>) -> Result<()> {
    let project_dir = std::env::var("THEATRE_PROJECT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    // 1. Validate tool name
    if !TOOLS.contains(&tool) {
        let mut error = serde_json::json!({
            "error": "unknown_tool",
            "message": format!("Unknown tool: '{tool}'"),
            "available_tools": TOOLS
        });
        theatre_feedback::append_json_notice(&mut error, &project_dir);
        println!("{error}");
        std::process::exit(2);
    }

    // 2. Parse params
    let params: Value = match json_arg {
        Some(s) => match serde_json::from_str(s) {
            Ok(v) => v,
            Err(e) => {
                let mut error = serde_json::json!({
                    "error": "invalid_json",
                    "message": format!("Invalid JSON: {e}"),
                });
                theatre_feedback::append_json_notice(&mut error, &project_dir);
                println!("{error}");
                std::process::exit(2);
            }
        },
        None => {
            // Check if stdin is piped
            if !std::io::stdin().is_terminal() {
                let mut input = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
                    .map_err(|e| anyhow::anyhow!("Failed to read stdin: {e}"))?;
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    serde_json::Value::Object(serde_json::Map::new())
                } else {
                    match serde_json::from_str(trimmed) {
                        Ok(v) => v,
                        Err(e) => {
                            let mut error = serde_json::json!({
                                "error": "invalid_json",
                                "message": format!("Invalid JSON from stdin: {e}"),
                            });
                            theatre_feedback::append_json_notice(&mut error, &project_dir);
                            println!("{error}");
                            std::process::exit(2);
                        }
                    }
                }
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            }
        }
    };

    // Reject impossible session workflows before connecting or mutating the game.
    if let Some(reason) = persistent_session_requirement(tool, &params) {
        let mut error = serde_json::json!({
            "error": "persistent_session_required",
            "message": reason,
            "hint": "Configure your MCP client to run `stage serve` and keep calls in that same session. Take spatial_snapshot before spatial_delta or an action with return_delta; watches and spatial_config updates also belong to that session. For reusable configuration defaults, edit stage.toml. Separate CLI calls never share session state.",
        });
        theatre_feedback::append_json_notice(&mut error, &project_dir);
        println!("{error}");
        std::process::exit(2);
    }

    if let Err(error) = prevalidate_before_connect(tool, &params) {
        let mut response = serde_json::json!({
            "error": "invalid_parameters",
            "code": error.code,
            "message": error.message,
        });
        theatre_feedback::append_json_notice(&mut response, &project_dir);
        println!("{response}");
        std::process::exit(2);
    }

    // 3. Initialize tracing at warn level — avoid polluting stderr for agents
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("stage=warn".parse().expect("valid directive")),
        )
        .try_init();

    // 4. Resolve port from env or default
    let port: u16 = std::env::var("THEATRE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // 5. Load TOML config if available
    let toml_port = config::load_toml_port(&project_dir);
    let resolved_port = toml_port.unwrap_or(port);
    let base_config = config::load_toml_config(&project_dir);

    // 6. Create session state
    let state = Arc::new(Mutex::new(tcp::SessionState {
        config: base_config,
        project_dashcam_config: config::load_toml_dashcam(&project_dir),
        project_dir: project_dir.clone(),
        ..Default::default()
    }));

    // Feedback is retained project evidence, not a live engine operation.
    if tool == "feedback" {
        let operation = serde_json::from_value::<theatre_feedback::Operation>(params);
        let result = operation.map_err(|e| e.to_string()).and_then(|operation| {
            theatre_feedback::Queue::open(&project_dir)
                .and_then(|queue| queue.execute(operation))
                .map_err(|e| e.to_string())
        });
        let (mut value, code) = match result {
            Ok(response) => (serde_json::to_value(response)?, 0),
            Err(message) => (
                serde_json::json!({"error": "feedback_error", "message": message}),
                1,
            ),
        };
        theatre_feedback::append_json_notice(&mut value, &project_dir);
        println!("{value}");
        std::process::exit(code);
    }

    // Saved-clip reads use the local path hint without contacting a game. When
    // no hint exists, a live addon may still resolve its authoritative user path.
    let retained_clip = tool == "clips"
        && serde_json::from_value::<stage_server::mcp::clips::ClipsParams>(params.clone())
            .is_ok_and(|params| !params.action.requires_live_runtime());
    let needs_connection = !retained_clip
        || stage_server::clip_analysis::resolve_clip_storage_path(&state)
            .await
            .is_err();

    // 7. Connect only when needed; saved data survives a connection failure.
    if needs_connection && let Err(e) = tcp::connect_once(&state, resolved_port).await {
        state.lock().await.connection_error = Some(e.to_string());
        if tool != "runtime_status" && !retained_clip {
            let mut error = serde_json::json!({
                "error": "connection_failed",
                "message": e.to_string(),
                "hint": format!(
                    "Ensure the Godot project is running with the Stage addon active on port {resolved_port}."
                )
            });
            theatre_feedback::append_json_notice(&mut error, &project_dir);
            println!("{error}");
            std::process::exit(1);
        }
    }

    // 8. Dispatch to handler
    let result = dispatch(tool, params, &state).await;

    // 9. Print result or error
    match result {
        Ok(json_str) => {
            let mut value: Value = serde_json::from_str(&json_str)?;
            theatre_feedback::append_json_notice(&mut value, &project_dir);
            println!("{value}");
            std::process::exit(0);
        }
        Err(e) => {
            let mut error = serde_json::json!({
                "error": "tool_error",
                "code": e.code,
                "message": e.message,
            });
            theatre_feedback::append_json_notice(&mut error, &project_dir);
            println!("{error}");
            std::process::exit(1);
        }
    }
}

/// One-shot calls cannot retain server-owned intent or a comparison baseline.
/// Config reads and addon-owned clip operations remain useful without persistence.
fn persistent_session_requirement(tool: &str, params: &Value) -> Option<&'static str> {
    match tool {
        "spatial_delta" => Some(
            "One-shot CLI calls have no delta baseline; a snapshot in a separate invocation cannot establish one.",
        ),
        "spatial_watch" => Some(
            "Watches belong to a persistent MCP session; one-shot CLI calls cannot add, remove, list, or clear that session's watches.",
        ),
        "spatial_action" if params.get("return_delta") == Some(&Value::Bool(true)) => Some(
            "return_delta requires a baseline in the same persistent session. No action was performed. Omit return_delta or set it to false for a one-shot action, then inspect or snapshot the result.",
        ),
        "spatial_config" => {
            // Let ordinary typed dispatch report malformed parameters. Null optional
            // values are not updates, just as in the shared handler.
            let p = serde_json::from_value::<stage_server::mcp::config::SpatialConfigParams>(
                params.clone(),
            )
            .ok()?;
            (p.static_patterns.is_some()
                || p.state_properties.is_some()
                || p.cluster_by.is_some()
                || p.bearing_format.is_some()
                || p.expose_internals.is_some()
                || p.poll_interval.is_some()
                || p.token_hard_cap.is_some())
                .then_some("spatial_config updates last only for the current server session and would be lost when this CLI call exits. Use spatial_config {} to read project defaults.")
        }
        _ => None,
    }
}

/// Reject malformed action requests before one-shot mode opens an engine
/// connection. This guarantees whole-sequence bounds are checked before any
/// possible input mutation.
fn prevalidate_before_connect(tool: &str, params: &Value) -> Result<(), McpError> {
    if tool == "spatial_action" {
        let typed =
            deserialize_params::<stage_server::mcp::action::SpatialActionParams>(params.clone())?;
        stage_server::mcp::action::build_action_request(&typed)?;
    }
    Ok(())
}

/// Deserialize params from a JSON Value into the typed struct.
fn deserialize_params<T: for<'de> serde::Deserialize<'de>>(value: Value) -> Result<T, McpError> {
    serde_json::from_value(value)
        .map_err(|e| McpError::invalid_params(format!("Invalid parameters: {e}"), None))
}

/// Dispatch tool name to the appropriate handler.
async fn dispatch(
    tool: &str,
    params: Value,
    state: &Arc<Mutex<tcp::SessionState>>,
) -> Result<String, McpError> {
    use stage_server::mcp;

    match tool {
        "viewport" => {
            let p = deserialize_params::<mcp::viewport::ViewportParams>(params)?;
            mcp::viewport::handle_viewport_cli(p, state).await
        }
        "runtime_status" => {
            let p = deserialize_params::<mcp::runtime_status::RuntimeStatusParams>(params)?;
            mcp::runtime_status::handle_runtime_status(p, state).await
        }
        "runtime_diagnostics" => {
            let p =
                deserialize_params::<mcp::runtime_diagnostics::RuntimeDiagnosticsParams>(params)?;
            mcp::runtime_diagnostics::handle_runtime_diagnostics(p, state).await
        }
        "spatial_snapshot" => {
            let p = deserialize_params::<mcp::snapshot::SpatialSnapshotParams>(params)?;
            mcp::handle_snapshot(p, state).await
        }
        "spatial_inspect" => {
            let p = deserialize_params::<mcp::inspect::SpatialInspectParams>(params)?;
            mcp::handle_inspect(p, state).await
        }
        "scene_tree" => {
            let p = deserialize_params::<mcp::scene_tree::SceneTreeToolParams>(params)?;
            mcp::handle_scene_tree(p, state).await
        }
        "spatial_action" => {
            let p = deserialize_params::<mcp::action::SpatialActionParams>(params)?;
            mcp::handle_action(p, state).await
        }
        "spatial_query" => {
            let p = deserialize_params::<mcp::query::SpatialQueryParams>(params)?;
            mcp::query::handle_spatial_query(p, state).await
        }
        "spatial_delta" => {
            let p = deserialize_params::<mcp::delta::SpatialDeltaParams>(params)?;
            mcp::delta::handle_spatial_delta(p, state).await
        }
        "spatial_watch" => {
            let p = deserialize_params::<mcp::watch::SpatialWatchParams>(params)?;
            mcp::watch::handle_spatial_watch(p, state).await
        }
        "spatial_config" => {
            let p = deserialize_params::<mcp::config::SpatialConfigParams>(params)?;
            mcp::config::handle_spatial_config(p, state).await
        }
        "clips" => {
            let p = deserialize_params::<mcp::clips::ClipsParams>(params)?;
            mcp::handle_clips_cli(p, state).await
        }
        _ => unreachable!("tool validated earlier"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_list_is_non_empty() {
        assert!(!TOOLS.is_empty());
    }

    #[test]
    fn cli_prevalidates_invalid_later_sequence_step_before_connect() {
        let error = prevalidate_before_connect(
            "spatial_action",
            &serde_json::json!({
                "action": "interaction_sequence",
                "steps": [
                    {"press": [{"action_name": "test_jump"}], "frames": 1},
                    {"release": ["test_jump"], "frames": 0}
                ]
            }),
        )
        .unwrap_err();
        assert!(error.message.contains("step 1"));
    }

    #[test]
    fn all_expected_tools_present() {
        let expected = [
            "spatial_snapshot",
            "spatial_inspect",
            "scene_tree",
            "spatial_action",
            "spatial_query",
            "spatial_delta",
            "spatial_watch",
            "spatial_config",
            "clips",
            "runtime_status",
            "runtime_diagnostics",
            "viewport",
        ];
        for tool in &expected {
            assert!(TOOLS.contains(tool), "missing tool: {tool}");
        }
    }
}
