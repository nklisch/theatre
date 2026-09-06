use std::{path::PathBuf, time::Duration};

use rmcp::model::{CallToolResult, ErrorData as McpError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, sleep};

use super::runtime_status::{RuntimeStatusParams, RuntimeStatusResponse, runtime_status};
use crate::{config, server::StageServer, tcp::SessionState};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectSelectParams {
    /// Absolute Godot project directory containing project.godot. Even selecting
    /// the same project again resets watches, baselines and session overrides.
    pub project_path: PathBuf,
    /// Listener port (1–65535). Overrides the selected project's stage.toml;
    /// omitted uses its TOML port or 9077, never the previous target's port.
    pub port: Option<u16>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProjectSelectResponse {
    #[serde(flatten)]
    pub status: RuntimeStatusResponse,
    /// Session state discarded by this selection; no per-project state is retained.
    pub cleared: Vec<&'static str>,
    pub message: &'static str,
}

impl StageServer {
    pub(crate) async fn select_project(
        &self,
        params: ProjectSelectParams,
    ) -> Result<CallToolResult, McpError> {
        // Validation is deliberately before teardown: a typo need not destroy
        // useful work on the old project. No filesystem allowlist is needed.
        if !params.project_path.is_absolute() {
            return Err(McpError::invalid_params(
                "project_path must be an absolute Godot project directory; selection was not changed.",
                None,
            ));
        }
        let project = std::fs::canonicalize(&params.project_path).map_err(|error| {
            McpError::invalid_params(
                format!(
                    "Cannot select project {}: {error}. Selection was not changed.",
                    params.project_path.display()
                ),
                None,
            )
        })?;
        if !project.join("project.godot").is_file() {
            return Err(McpError::invalid_params(
                format!(
                    "{} does not contain project.godot; selection was not changed.",
                    project.display()
                ),
                None,
            ));
        }
        let port = params
            .port
            .or_else(|| config::load_toml_port(&project))
            .unwrap_or(9077);
        if port == 0 {
            return Err(McpError::invalid_params(
                "port must be between 1 and 65535; selection was not changed.",
                None,
            ));
        }
        let replacement = SessionState {
            config: config::load_toml_config(&project),
            project_dashcam_config: config::load_toml_dashcam(&project),
            project_dir: project.clone(),
            port,
            ..Default::default()
        };
        let exclusive = self.connection.operations.clone().write_owned().await;
        let _exclusive = self.connection
            .connect(&self.state, port, Some(replacement), exclusive)
            .await.map_err(|error| McpError::internal_error(
                format!("Project selection task failed: {error}. Check runtime_status before further actions."), None
            ))?;

        // A bounded initial opportunity to connect makes the switch useful when
        // the game is already running. The owned reconnect task continues even
        // if this wait expires or the MCP request is cancelled.
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = self.state.lock().await;
            if state.connected || state.connection_error.is_some() || Instant::now() >= deadline {
                break;
            }
            drop(state);
            sleep(Duration::from_millis(25)).await;
        }
        let status = runtime_status(RuntimeStatusParams {}, &self.state).await?;
        let mut response = ProjectSelectResponse {
            status,
            cleared: vec![
                "watches",
                "delta_baseline",
                "spatial_index",
                "session_overrides",
                "clip_storage_location",
            ],
            message: "Project selected; previous session state was discarded, even if the project is unchanged. Take a fresh spatial_snapshot before spatial_delta or indexed spatial queries; recreate watches and session overrides as needed. If disconnected, this project remains selected and Stage retries it, never the previous target. Running games, their recording buffers and saved clips were not stopped or deleted. Director still selects project_path per call; client feedback-hook environments are unchanged.",
        };
        response.status.budget.used = stage_core::budget::estimate_tokens(
            stage_protocol::mcp_helpers::serialize_response(&response)?.len(),
        );
        let mut result = stage_protocol::mcp_helpers::serialize_response(&response)
            .and_then(stage_protocol::mcp_helpers::structured_json);
        theatre_feedback::mcp::append_notice(&mut result, &project);
        result
    }
}
