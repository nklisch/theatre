pub mod node;
pub mod scene;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use rmcp::tool_router;

use crate::oneshot::run_oneshot;
use crate::resolve::{resolve_godot_bin, validate_project_path};
use crate::server::DirectorServer;

use node::{NodeAddParams, NodeRemoveParams, NodeSetPropertiesParams};
use scene::{SceneCreateParams, SceneReadParams};

// ---------------------------------------------------------------------------
// Shared MCP helpers
// ---------------------------------------------------------------------------

/// Run an operation via headless Godot and return the parsed result data.
/// Handles godot resolution, project validation, subprocess, and JSON parsing.
async fn run_operation(
    project_path: &str,
    operation: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    let godot = resolve_godot_bin().map_err(McpError::from)?;
    let project = std::path::Path::new(project_path);
    validate_project_path(project).map_err(McpError::from)?;

    let result = run_oneshot(&godot, project, operation, params)
        .await
        .map_err(McpError::from)?;

    result.into_data().map_err(McpError::from)
}

fn serialize_params<T: serde::Serialize>(params: &T) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(params).map_err(|e| {
        McpError::internal_error(format!("Param serialization error: {e}"), None)
    })
}

fn serialize_response<T: serde::Serialize>(response: &T) -> Result<String, McpError> {
    serde_json::to_string(response).map_err(|e| {
        McpError::internal_error(format!("Response serialization error: {e}"), None)
    })
}

// ---------------------------------------------------------------------------
// Tool router
// ---------------------------------------------------------------------------

#[tool_router(vis = "pub")]
impl DirectorServer {
    #[tool(
        name = "scene_create",
        description = "Create a new Godot scene file (.tscn) with a specified root node type. \
            Always use this tool instead of creating .tscn files directly — the scene \
            serialization format is fragile and hand-editing will produce corrupt scenes."
    )]
    pub async fn scene_create(
        &self,
        Parameters(params): Parameters<SceneCreateParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "scene_create", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "scene_read",
        description = "Read the full node tree of a Godot scene file (.tscn) with types, \
            properties, and hierarchy. Use this to understand existing scene structure before \
            making modifications."
    )]
    pub async fn scene_read(
        &self,
        Parameters(params): Parameters<SceneReadParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "scene_read", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "node_add",
        description = "Add a node to a Godot scene file (.tscn). Optionally set initial \
            properties. Always use this tool instead of editing .tscn files directly — the scene \
            serialization format is fragile and hand-editing will produce corrupt scenes."
    )]
    pub async fn node_add(
        &self,
        Parameters(params): Parameters<NodeAddParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "node_add", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "node_set_properties",
        description = "Set properties on a node in a Godot scene file (.tscn). Handles type \
            conversion automatically (Vector2, Vector3, Color, NodePath, resource paths). \
            Always use this tool instead of editing .tscn files directly."
    )]
    pub async fn node_set_properties(
        &self,
        Parameters(params): Parameters<NodeSetPropertiesParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "node_set_properties", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "node_remove",
        description = "Remove a node (and all its children) from a Godot scene file (.tscn). \
            Always use this tool instead of editing .tscn files directly."
    )]
    pub async fn node_remove(
        &self,
        Parameters(params): Parameters<NodeRemoveParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&params.project_path, "node_remove", &op_params).await?;
        serialize_response(&data)
    }
}
