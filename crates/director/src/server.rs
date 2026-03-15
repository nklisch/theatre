use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::tool_handler;
use schemars::JsonSchema;

use crate::backend::Backend;
use crate::responses::{
    AnimationAddTrackResponse, AnimationCreateResponse, AnimationReadResponse,
    AnimationRemoveTrackResponse, BatchResponse, ExportMeshLibraryResponse, GridMapClearResponse,
    GridMapGetCellsResponse, GridMapSetCellsResponse, NodeAddResponse, NodeFindResponse,
    NodeRemoveResponse, NodeReparentResponse, NodeSetGroupsResponse, NodeSetMetaResponse,
    NodeSetPropertiesResponse, NodeSetScriptResponse, PhysicsSetLayerNamesResponse,
    PhysicsSetLayersResponse, ResourceCreateResponse, ResourceDuplicateResponse,
    ResourceReadResponse, SceneAddInstanceResponse, SceneCreateResponse, SceneDiffResponse,
    SceneListResponse, SceneReadResponse, ShapeCreateResponse, SignalConnectionResponse,
    SignalListResponse, TileMapClearResponse, TileMapGetCellsResponse, TileMapSetCellsResponse,
    UidGetResponse, UidUpdateProjectResponse, VisualShaderCreateResponse,
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

#[tool_handler]
impl ServerHandler for DirectorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "director".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                ..Default::default()
            },
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
