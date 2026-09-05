use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stage_protocol::mcp_helpers::{deserialize_response, serialize_params};

use crate::backend::Backend;
use crate::resolve::validate_project_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EditorRunAction {
    Start,
    Stop,
    Restart,
    Status,
}

/// Parameters for native selected-scene run control in an existing Godot editor.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditorRunParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,
    /// Lifecycle action. Start and restart require scene_path; stop and status reject it.
    pub action: EditorRunAction,
    /// Saved scene path relative to the project. Valid only for start and restart.
    #[serde(default)]
    pub scene_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditorRunResponse {
    pub action: EditorRunAction,
    /// Selected saved scene for a launch, or the scene that was playing for stop/status.
    pub scene_path: String,
    /// True only when this call synchronously asked EditorInterface to launch a scene.
    pub launch_requested: bool,
    /// Native EditorInterface play state immediately after the action.
    pub game_running: bool,
    /// Native EditorInterface playing scene after the action; empty when stopped.
    pub playing_scene: String,
}

pub async fn run_editor(
    backend: &Backend,
    params: &EditorRunParams,
) -> Result<EditorRunResponse, McpError> {
    validate_action(params)?;
    let project = std::path::Path::new(&params.project_path);
    validate_project_path(project).map_err(McpError::from)?;
    let op_params = serialize_params(params)?;
    let result = backend
        .run_editor_operation(project, "editor_run", &op_params)
        .await
        .map_err(McpError::from)?;
    let data = result.into_data().map_err(McpError::from)?;
    deserialize_response(data)
}

fn validate_action(params: &EditorRunParams) -> Result<(), McpError> {
    match (params.action, params.scene_path.as_deref()) {
        (EditorRunAction::Start | EditorRunAction::Restart, None | Some("")) => Err(
            McpError::invalid_params("scene_path is required for start and restart", None),
        ),
        (EditorRunAction::Stop | EditorRunAction::Status, Some(_)) => Err(
            McpError::invalid_params("scene_path is only valid for start and restart", None),
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(action: EditorRunAction, scene_path: Option<&str>) -> EditorRunParams {
        EditorRunParams {
            project_path: "/project".into(),
            action,
            scene_path: scene_path.map(str::to_owned),
        }
    }

    #[test]
    fn action_specific_scene_path_validation_is_explicit() {
        assert!(validate_action(&params(EditorRunAction::Start, Some("main.tscn"))).is_ok());
        assert!(validate_action(&params(EditorRunAction::Restart, None)).is_err());
        assert!(validate_action(&params(EditorRunAction::Stop, Some("main.tscn"))).is_err());
        assert!(validate_action(&params(EditorRunAction::Status, None)).is_ok());
    }

    #[test]
    fn omitted_scene_path_is_not_serialized_as_null() {
        let value = serde_json::to_value(params(EditorRunAction::Stop, None)).unwrap();
        assert!(!value.as_object().unwrap().contains_key("scene_path"));
    }
}
