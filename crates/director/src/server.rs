use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use schemars::JsonSchema;

use crate::backend::Backend;
use crate::mcp::editor_run::EditorRunResponse;
use crate::mcp::engine_api::EngineApiResponse;
use crate::responses::{
    AnimationAddTrackResponse, AnimationCreateResponse, AnimationReadResponse,
    AnimationRemoveTrackResponse, AutoloadAddResponse, AutoloadRemoveResponse, BatchResponse,
    EditorStatusResponse, ExportMeshLibraryResponse, GridMapClearResponse, GridMapGetCellsResponse,
    GridMapSetCellsResponse, NodeAddResponse, NodeFindResponse, NodeRemoveResponse,
    NodeReparentResponse, NodeSetGroupsResponse, NodeSetMetaResponse, NodeSetPropertiesResponse,
    NodeSetScriptResponse, PhysicsSetLayerNamesResponse, PhysicsSetLayersResponse,
    ProjectReloadResponse, ProjectSettingsSetResponse, ResourceCreateResponse,
    ResourceDuplicateResponse, ResourceReadResponse, SceneAddInstanceResponse, SceneCreateResponse,
    SceneDiffResponse, SceneListResponse, SceneReadResponse, SceneSaveResponse,
    ShapeCreateResponse, SignalConnectionResponse, SignalListResponse, TileMapClearResponse,
    TileMapGetCellsResponse, TileMapSetCellsResponse, UidGetResponse, UidUpdateProjectResponse,
    VisualShaderCreateResponse,
};

#[derive(Clone)]
pub struct DirectorServer {
    pub tool_router: ToolRouter<Self>,
    pub backend: Arc<Backend>,
}

fn attach_output_schema<T: JsonSchema + 'static>(
    router: &mut ToolRouter<DirectorServer>,
    tool_name: &str,
) {
    if let Some(route) = router.map.get_mut(tool_name) {
        route.attr = route.attr.clone().with_output_schema::<T>();
    }
}

/// Post-process all tool schemas in the router, replacing bare `true` schema
/// values with `{}` for MCP client compatibility.
fn sanitize_schemas(router: &mut ToolRouter<DirectorServer>) {
    for route in router.map.values_mut() {
        if let Some(ref schema) = route.attr.output_schema {
            let mut value = serde_json::Value::Object(schema.as_ref().clone());
            stage_protocol::mcp_helpers::replace_bool_schemas(&mut value);
            if let serde_json::Value::Object(map) = value {
                route.attr.output_schema = Some(Arc::new(map));
            }
        }
    }
}

impl DirectorServer {
    pub fn new() -> Self {
        let mut router = Self::tool_router();

        attach_output_schema::<theatre_feedback::Response>(&mut router, "feedback");
        attach_output_schema::<SceneSaveResponse>(&mut router, "scene_save");
        attach_output_schema::<SceneCreateResponse>(&mut router, "scene_create");
        attach_output_schema::<SceneReadResponse>(&mut router, "scene_read");
        attach_output_schema::<SceneListResponse>(&mut router, "scene_list");
        attach_output_schema::<SceneAddInstanceResponse>(&mut router, "scene_add_instance");
        attach_output_schema::<SceneDiffResponse>(&mut router, "scene_diff");
        attach_output_schema::<NodeAddResponse>(&mut router, "node_add");
        attach_output_schema::<NodeSetPropertiesResponse>(&mut router, "node_set_properties");
        attach_output_schema::<NodeRemoveResponse>(&mut router, "node_remove");
        attach_output_schema::<NodeReparentResponse>(&mut router, "node_reparent");
        attach_output_schema::<NodeSetGroupsResponse>(&mut router, "node_set_groups");
        attach_output_schema::<NodeSetScriptResponse>(&mut router, "node_set_script");
        attach_output_schema::<NodeSetMetaResponse>(&mut router, "node_set_meta");
        attach_output_schema::<NodeFindResponse>(&mut router, "node_find");
        attach_output_schema::<EngineApiResponse>(&mut router, "engine_api");
        attach_output_schema::<ResourceReadResponse>(&mut router, "resource_read");
        attach_output_schema::<ResourceCreateResponse>(&mut router, "material_create");
        attach_output_schema::<ShapeCreateResponse>(&mut router, "shape_create");
        attach_output_schema::<ResourceCreateResponse>(&mut router, "style_box_create");
        attach_output_schema::<ResourceDuplicateResponse>(&mut router, "resource_duplicate");
        attach_output_schema::<TileMapSetCellsResponse>(&mut router, "tilemap_set_cells");
        attach_output_schema::<TileMapGetCellsResponse>(&mut router, "tilemap_get_cells");
        attach_output_schema::<TileMapClearResponse>(&mut router, "tilemap_clear");
        attach_output_schema::<GridMapSetCellsResponse>(&mut router, "gridmap_set_cells");
        attach_output_schema::<GridMapGetCellsResponse>(&mut router, "gridmap_get_cells");
        attach_output_schema::<GridMapClearResponse>(&mut router, "gridmap_clear");
        attach_output_schema::<AnimationCreateResponse>(&mut router, "animation_create");
        attach_output_schema::<AnimationAddTrackResponse>(&mut router, "animation_add_track");
        attach_output_schema::<AnimationReadResponse>(&mut router, "animation_read");
        attach_output_schema::<AnimationRemoveTrackResponse>(&mut router, "animation_remove_track");
        attach_output_schema::<PhysicsSetLayersResponse>(&mut router, "physics_set_layers");
        attach_output_schema::<PhysicsSetLayerNamesResponse>(
            &mut router,
            "physics_set_layer_names",
        );
        attach_output_schema::<VisualShaderCreateResponse>(&mut router, "visual_shader_create");
        attach_output_schema::<BatchResponse>(&mut router, "batch");
        attach_output_schema::<UidGetResponse>(&mut router, "uid_get");
        attach_output_schema::<UidUpdateProjectResponse>(&mut router, "uid_update_project");
        attach_output_schema::<ExportMeshLibraryResponse>(&mut router, "export_mesh_library");
        attach_output_schema::<AutoloadAddResponse>(&mut router, "autoload_add");
        attach_output_schema::<AutoloadRemoveResponse>(&mut router, "autoload_remove");
        attach_output_schema::<ProjectSettingsSetResponse>(&mut router, "project_settings_set");
        attach_output_schema::<ProjectReloadResponse>(&mut router, "project_reload");
        attach_output_schema::<EditorRunResponse>(&mut router, "editor_run");
        attach_output_schema::<EditorStatusResponse>(&mut router, "editor_status");
        attach_output_schema::<SignalConnectionResponse>(&mut router, "signal_connect");
        attach_output_schema::<SignalConnectionResponse>(&mut router, "signal_disconnect");
        attach_output_schema::<SignalListResponse>(&mut router, "signal_list");

        sanitize_schemas(&mut router);

        Self {
            tool_router: router,
            backend: Arc::new(Backend::new()),
        }
    }
}

impl Default for DirectorServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandler for DirectorServer {
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        let project = request
            .arguments
            .as_ref()
            .and_then(|args| args.get("project_path"))
            .and_then(|value| value.as_str())
            .map(std::path::PathBuf::from);
        let call = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        let mut result = self.tool_router.call(call).await;
        if let Some(project) = project {
            theatre_feedback::mcp::append_notice(&mut result, &project);
        }
        result
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        Ok(rmcp::model::ListToolsResult {
            tools: self.tool_router.list_all(),
            meta: None,
            next_cursor: None,
        })
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        self.tool_router.get(name).cloned()
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "director".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some("Edits to any open scene are native-undoable and remain unsaved, including batch entries. Use scene_save to save only that scene without flushing external resources; its native dirty marker may remain. Closed-scene and headless edits save to disk. Read persistence in results and error data; batches are sequential, not atomic.".into()),
            ..Default::default()
        }
    }
}
