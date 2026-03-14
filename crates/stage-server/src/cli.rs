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
];

/// Default TCP port for connecting to the Godot addon.
const DEFAULT_PORT: u16 = 9077;

/// Entry point for CLI one-shot mode.
///
/// `tool` — the tool name to invoke.
/// `json_arg` — optional JSON string from CLI arg; if None and stdin is piped, read from stdin.
pub async fn run(tool: &str, json_arg: Option<&str>) -> Result<()> {
    // 1. Validate tool name
    if !TOOLS.contains(&tool) {
        let error = serde_json::json!({
            "error": "unknown_tool",
            "message": format!("Unknown tool: '{tool}'"),
            "available_tools": TOOLS
        });
        println!("{error}");
        std::process::exit(2);
    }

    // 2. Parse params
    let params: Value = match json_arg {
        Some(s) => match serde_json::from_str(s) {
            Ok(v) => v,
            Err(e) => {
                let error = serde_json::json!({
                    "error": "invalid_json",
                    "message": format!("Invalid JSON: {e}"),
                });
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
                            let error = serde_json::json!({
                                "error": "invalid_json",
                                "message": format!("Invalid JSON from stdin: {e}"),
                            });
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

    // 3. Initialize tracing at warn level — avoid polluting stderr for agents
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("stage=warn".parse().expect("valid directive")),
        )
        .try_init();

    // 4. Resolve port from env or default
    let port: u16 = match std::env::var("THEATRE_PORT") {
        Ok(v) => v.parse().unwrap_or(DEFAULT_PORT),
        Err(_) => match std::env::var("SPECTATOR_PORT") {
            Ok(v) => v.parse().unwrap_or(DEFAULT_PORT),
            Err(_) => DEFAULT_PORT,
        },
    };

    // 5. Load TOML config if available
    let project_dir = std::env::var("SPECTATOR_PROJECT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let toml_port = config::load_toml_port(&project_dir);
    let resolved_port = toml_port.unwrap_or(port);
    let base_config = config::load_toml_config(&project_dir);

    // 6. Create session state
    let state = Arc::new(Mutex::new(tcp::SessionState {
        config: base_config,
        ..Default::default()
    }));

    // 7. Connect to addon
    if let Err(e) = tcp::connect_once(&state, resolved_port).await {
        let error = serde_json::json!({
            "error": "connection_failed",
            "message": e.to_string(),
            "hint": format!(
                "Ensure the Godot project is running with the Stage addon active on port {resolved_port}."
            )
        });
        println!("{error}");
        std::process::exit(1);
    }

    // 8. Dispatch to handler
    let result = dispatch(tool, params, &state).await;

    // 9. Print result or error
    match result {
        Ok(json_str) => {
            println!("{json_str}");
            std::process::exit(0);
        }
        Err(e) => {
            let error = serde_json::json!({
                "error": "tool_error",
                "code": e.code,
                "message": e.message,
            });
            println!("{error}");
            std::process::exit(1);
        }
    }
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
        ];
        for tool in &expected {
            assert!(TOOLS.contains(tool), "missing tool: {tool}");
        }
    }
}
