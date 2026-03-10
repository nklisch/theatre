pub mod animation;
pub mod gridmap;
pub mod node;
pub mod resource;
pub mod scene;
pub mod tilemap;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use rmcp::tool_router;

use crate::backend::Backend;
use crate::resolve::{resolve_godot_bin, validate_project_path};
use crate::server::DirectorServer;

use animation::{
    AnimationAddTrackParams, AnimationCreateParams, AnimationReadParams, AnimationRemoveTrackParams,
};
use gridmap::{GridMapClearParams, GridMapGetCellsParams, GridMapSetCellsParams};
use node::{NodeAddParams, NodeRemoveParams, NodeReparentParams, NodeSetPropertiesParams};
use resource::{
    MaterialCreateParams, ResourceDuplicateParams, ResourceReadParams, ShapeCreateParams,
    StyleBoxCreateParams,
};
use scene::{SceneAddInstanceParams, SceneCreateParams, SceneListParams, SceneReadParams};
use tilemap::{TileMapClearParams, TileMapGetCellsParams, TileMapSetCellsParams};

// ---------------------------------------------------------------------------
// Shared MCP helpers
// ---------------------------------------------------------------------------

/// Run an operation via the best available backend and return the parsed result data.
/// Handles godot resolution, project validation, and backend routing.
async fn run_operation(
    backend: &Backend,
    project_path: &str,
    operation: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    let godot = resolve_godot_bin().map_err(McpError::from)?;
    let project = std::path::Path::new(project_path);
    validate_project_path(project).map_err(McpError::from)?;

    let result = backend
        .run_operation(&godot, project, operation, params)
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
        let data = run_operation(&self.backend, &params.project_path, "scene_create", &op_params).await?;
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
        let data = run_operation(&self.backend, &params.project_path, "scene_read", &op_params).await?;
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
        let data = run_operation(&self.backend, &params.project_path, "node_add", &op_params).await?;
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
        let data = run_operation(&self.backend, &params.project_path, "node_set_properties", &op_params).await?;
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
        let data = run_operation(&self.backend, &params.project_path, "node_remove", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "scene_list",
        description = "List all Godot scene files (.tscn) in the project or a subdirectory, \
            with root node type and node count for each scene."
    )]
    pub async fn scene_list(
        &self,
        Parameters(params): Parameters<SceneListParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "scene_list", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "scene_add_instance",
        description = "Add a scene instance (reference) as a child node in another Godot scene. \
            The instanced scene is linked, not copied — changes to the source scene propagate. \
            Always use this tool instead of editing .tscn files directly."
    )]
    pub async fn scene_add_instance(
        &self,
        Parameters(params): Parameters<SceneAddInstanceParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "scene_add_instance", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "node_reparent",
        description = "Move a node to a new parent within the same Godot scene. Optionally \
            rename the node during the move. Always use this tool instead of editing .tscn \
            files directly."
    )]
    pub async fn node_reparent(
        &self,
        Parameters(params): Parameters<NodeReparentParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "node_reparent", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "resource_read",
        description = "Read a Godot resource file (.tres, .res) and return its type and \
            properties as structured data. For scene files (.tscn), prefer scene_read which \
            returns the full node tree."
    )]
    pub async fn resource_read(
        &self,
        Parameters(params): Parameters<ResourceReadParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "resource_read", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "material_create",
        description = "Create a Godot material resource (.tres). Supports StandardMaterial3D, \
            ORMMaterial3D, ShaderMaterial, CanvasItemMaterial, ParticleProcessMaterial, and \
            any ClassDB Material subclass. Always use this instead of hand-writing .tres files."
    )]
    pub async fn material_create(
        &self,
        Parameters(params): Parameters<MaterialCreateParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "material_create", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "shape_create",
        description = "Create a Godot collision shape resource. Supports 3D shapes (BoxShape3D, \
            SphereShape3D, CapsuleShape3D, etc.) and 2D shapes (CircleShape2D, RectangleShape2D, \
            etc.). Can save as .tres and/or attach directly to a CollisionShape node in a scene. \
            At least one of save_path or scene attachment (scene_path + node_path) is required."
    )]
    pub async fn shape_create(
        &self,
        Parameters(params): Parameters<ShapeCreateParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "shape_create", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "style_box_create",
        description = "Create a Godot StyleBox resource (.tres) for UI theming. Supports \
            StyleBoxFlat, StyleBoxTexture, StyleBoxLine, and StyleBoxEmpty. Always use this \
            instead of hand-writing .tres files."
    )]
    pub async fn style_box_create(
        &self,
        Parameters(params): Parameters<StyleBoxCreateParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "style_box_create", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "resource_duplicate",
        description = "Duplicate a Godot resource file (.tres, .res) to a new path, optionally \
            applying property overrides. Use deep_copy to make nested sub-resources independent."
    )]
    pub async fn resource_duplicate(
        &self,
        Parameters(params): Parameters<ResourceDuplicateParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "resource_duplicate", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "tilemap_set_cells",
        description = "Set cells on a TileMapLayer node in a Godot scene. Each cell is placed by \
            grid coordinates, TileSet source ID, and atlas coordinates. The TileMapLayer must already \
            have a TileSet resource assigned. Always use this instead of editing .tscn files directly."
    )]
    pub async fn tilemap_set_cells(
        &self,
        Parameters(params): Parameters<TileMapSetCellsParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "tilemap_set_cells", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "tilemap_get_cells",
        description = "Read used cells from a TileMapLayer node in a Godot scene. Returns cell \
            coordinates, source IDs, atlas coordinates, and the used rect. Optionally filter by \
            region or source ID."
    )]
    pub async fn tilemap_get_cells(
        &self,
        Parameters(params): Parameters<TileMapGetCellsParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "tilemap_get_cells", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "tilemap_clear",
        description = "Clear cells from a TileMapLayer node in a Godot scene. Optionally specify \
            a region to clear only cells within that rectangle; omit to clear all cells."
    )]
    pub async fn tilemap_clear(
        &self,
        Parameters(params): Parameters<TileMapClearParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "tilemap_clear", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "gridmap_set_cells",
        description = "Set cells in a GridMap node in a Godot scene. Each cell is placed by 3D grid \
            position and MeshLibrary item index. The GridMap must already have a MeshLibrary resource \
            assigned. Always use this instead of editing .tscn files directly."
    )]
    pub async fn gridmap_set_cells(
        &self,
        Parameters(params): Parameters<GridMapSetCellsParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "gridmap_set_cells", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "gridmap_get_cells",
        description = "Read used cells from a GridMap node in a Godot scene. Returns cell positions, \
            MeshLibrary item indices, and orientations. Optionally filter by bounds or item."
    )]
    pub async fn gridmap_get_cells(
        &self,
        Parameters(params): Parameters<GridMapGetCellsParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "gridmap_get_cells", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "gridmap_clear",
        description = "Clear cells from a GridMap node in a Godot scene. Optionally specify bounds \
            to clear only cells within that box; omit to clear all cells."
    )]
    pub async fn gridmap_clear(
        &self,
        Parameters(params): Parameters<GridMapClearParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "gridmap_clear", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "animation_create",
        description = "Create a Godot Animation resource (.tres) with specified length and \
            loop mode. The animation starts empty — use animation_add_track to add tracks \
            and keyframes. Always use this instead of hand-writing .tres files."
    )]
    pub async fn animation_create(
        &self,
        Parameters(params): Parameters<AnimationCreateParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "animation_create", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "animation_add_track",
        description = "Add a track with keyframes to a Godot Animation resource. Supports \
            value, position_3d, rotation_3d, scale_3d, blend_shape, method, and bezier \
            track types. Node paths are relative to the AnimationPlayer that will play this \
            animation. Always use this instead of editing .tres files directly."
    )]
    pub async fn animation_add_track(
        &self,
        Parameters(params): Parameters<AnimationAddTrackParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "animation_add_track", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "animation_read",
        description = "Read a Godot Animation resource (.tres) and return its full structure: \
            length, loop mode, and all tracks with their keyframes serialized as JSON. Use \
            this to inspect animation structure before making modifications."
    )]
    pub async fn animation_read(
        &self,
        Parameters(params): Parameters<AnimationReadParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "animation_read", &op_params).await?;
        serialize_response(&data)
    }

    #[tool(
        name = "animation_remove_track",
        description = "Remove a track from a Godot Animation resource by index or node path. \
            When removing by node_path, all tracks matching that path are removed. Always \
            use this instead of editing .tres files directly."
    )]
    pub async fn animation_remove_track(
        &self,
        Parameters(params): Parameters<AnimationRemoveTrackParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(&self.backend, &params.project_path, "animation_remove_track", &op_params).await?;
        serialize_response(&data)
    }
}
