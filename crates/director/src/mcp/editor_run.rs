use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stage_protocol::mcp_helpers::{deserialize_response, serialize_params};

use crate::backend::Backend;
pub use crate::presentation::{Presentation, PresentationOutcome, PresentationResult};
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
    /// Optional window presentation intent. Omission preserves legacy launch behavior.
    #[serde(default)]
    pub presentation: Option<Presentation>,
    /// Saved scene path relative to the project. Valid only for start and restart.
    #[serde(default)]
    pub scene_path: Option<String>,
    /// Per-run Theatre options. Omitted launches stay off unless explicit local preferences enable them.
    #[serde(default)]
    pub launch: Option<stage_protocol::capture::LaunchOptions>,
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
    /// Requested scene, distinct from the native bootstrap playing_scene. Empty when stopped or unknown.
    #[serde(default)]
    pub target_scene: String,
    /// Present only when the caller explicitly selected a presentation mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationResult>,
    /// Resolved channels, readiness, triggers and preference provenance for this launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<stage_protocol::capture::LaunchConfig>,
}

pub async fn run_editor(
    backend: &Backend,
    params: &EditorRunParams,
) -> Result<EditorRunResponse, McpError> {
    validate_action(params)?;
    let project = std::path::Path::new(&params.project_path);
    validate_project_path(project).map_err(McpError::from)?;
    let prepared = match params.presentation {
        Some(mode @ (Presentation::Automated | Presentation::Deferred)) => {
            Some(prepare_presentation(backend, project, mode).await?)
        }
        _ => None,
    };
    let op_params = serialize_params(params)?;
    let result = backend
        .run_editor_operation(project, "editor_run", &op_params)
        .await
        .map_err(McpError::from)?;
    let mut data = result.into_data().map_err(McpError::from)?;
    if let Some(launch) = data.get_mut("launch") {
        stage_protocol::capture::normalize_godot_integers(launch);
    }
    let response: EditorRunResponse = deserialize_response(data)?;
    let presentation = match (params.presentation, prepared) {
        (Some(Presentation::Interactive), None) => Some(PresentationResult::interactive()),
        (Some(_), Some(prepared)) => Some(prepared.finish().await),
        (None, None) => None,
        _ => None,
    };
    Ok(with_presentation(response, presentation))
}

fn with_presentation(
    mut response: EditorRunResponse,
    presentation: Option<PresentationResult>,
) -> EditorRunResponse {
    response.presentation = presentation;
    response
}

async fn prepare_presentation(
    backend: &Backend,
    project: &std::path::Path,
    mode: Presentation,
) -> Result<crate::presentation::PreparedPresentation, McpError> {
    #[cfg(windows)]
    let editor_process_id = backend
        .editor_process_id(project)
        .await
        .map_err(McpError::from)?;
    #[cfg(not(windows))]
    let editor_process_id = {
        let _ = backend;
        let _ = project;
        0
    };
    crate::presentation::prepare(mode, editor_process_id)
        .map_err(|reason| McpError::invalid_params(reason, None))
}

fn validate_action(params: &EditorRunParams) -> Result<(), McpError> {
    if params.presentation.is_some()
        && matches!(
            params.action,
            EditorRunAction::Stop | EditorRunAction::Status
        )
    {
        return Err(McpError::invalid_params(
            "presentation is only valid for start and restart",
            None,
        ));
    }
    if params.launch.is_some()
        && matches!(
            params.action,
            EditorRunAction::Stop | EditorRunAction::Status
        )
    {
        return Err(McpError::invalid_params(
            "launch is only valid for start and restart",
            None,
        ));
    }
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
            presentation: None,
            scene_path: scene_path.map(str::to_owned),
            launch: None,
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
        assert!(!value.as_object().unwrap().contains_key("presentation"));
    }

    #[test]
    fn presentation_is_independent_from_capture_options() {
        let without_capture: EditorRunParams = serde_json::from_value(serde_json::json!({
            "project_path":"/project", "action":"start", "scene_path":"main.tscn",
            "presentation":"automated"
        }))
        .unwrap();
        let with_human_capture: EditorRunParams = serde_json::from_value(serde_json::json!({
            "project_path":"/project", "action":"start", "scene_path":"main.tscn",
            "presentation":"automated", "launch":{"operator":"human"}
        }))
        .unwrap();
        assert_eq!(without_capture.presentation, Some(Presentation::Automated));
        assert_eq!(
            with_human_capture.presentation,
            Some(Presentation::Automated)
        );
    }

    #[test]
    fn stop_and_status_do_not_accept_presentation_checks() {
        for action in [EditorRunAction::Stop, EditorRunAction::Status] {
            let mut request = params(action, None);
            request.presentation = Some(Presentation::Automated);
            assert!(validate_action(&request).is_err());
        }
    }

    #[test]
    fn degraded_presentation_preserves_launch_and_running_facts() {
        let response = EditorRunResponse {
            action: EditorRunAction::Start,
            scene_path: "main.tscn".into(),
            launch_requested: true,
            game_running: true,
            playing_scene: "main.tscn".into(),
            target_scene: "main.tscn".into(),
            presentation: None,
            launch: None,
        };
        let response = with_presentation(
            response,
            Some(PresentationResult {
                mode: Presentation::Automated,
                outcome: PresentationOutcome::Degraded,
                reason: Some("window operation failed; stop before fallback".into()),
            }),
        );
        assert!(response.launch_requested);
        assert!(response.game_running);
        assert_eq!(
            response.presentation.unwrap().outcome,
            PresentationOutcome::Degraded
        );
    }
}
