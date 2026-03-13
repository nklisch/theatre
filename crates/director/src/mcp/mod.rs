pub mod animation;
pub mod defaults;
pub mod gridmap;
pub mod meta;
pub mod node;
pub mod physics;
pub mod project;
pub mod resource;
pub mod scene;
pub mod shader;
pub mod signal;
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
use meta::{BatchParams, SceneDiffParams};
use node::{
    NodeAddParams, NodeFindParams, NodeRemoveParams, NodeReparentParams, NodeSetGroupsParams,
    NodeSetMetaParams, NodeSetPropertiesParams, NodeSetScriptParams,
};
use physics::{PhysicsSetLayerNamesParams, PhysicsSetLayersParams};
use project::{ExportMeshLibraryParams, UidGetParams, UidUpdateProjectParams};
use resource::{
    MaterialCreateParams, ResourceDuplicateParams, ResourceReadParams, ShapeCreateParams,
    StyleBoxCreateParams,
};
use scene::{SceneAddInstanceParams, SceneCreateParams, SceneListParams, SceneReadParams};
use shader::VisualShaderCreateParams;
use signal::{SignalConnectParams, SignalDisconnectParams, SignalListParams};
use tilemap::{TileMapClearParams, TileMapGetCellsParams, TileMapSetCellsParams};

use spectator_protocol::mcp_helpers::{serialize_params, serialize_response};

// ---------------------------------------------------------------------------
// Shared MCP helpers
// ---------------------------------------------------------------------------

macro_rules! director_tool {
    ($self:expr, $params:expr, $op:expr) => {{
        let op_params = serialize_params(&$params)?;
        let data = run_operation(&$self.backend, &$params.project_path, $op, &op_params).await?;
        serialize_response(&data)
    }};
}

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
        director_tool!(self, params, "scene_create")
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
        director_tool!(self, params, "scene_read")
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
        director_tool!(self, params, "node_add")
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
        director_tool!(self, params, "node_set_properties")
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
        director_tool!(self, params, "node_remove")
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
        director_tool!(self, params, "scene_list")
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
        director_tool!(self, params, "scene_add_instance")
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
        director_tool!(self, params, "node_reparent")
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
        director_tool!(self, params, "resource_read")
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
        director_tool!(self, params, "material_create")
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
        director_tool!(self, params, "shape_create")
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
        director_tool!(self, params, "style_box_create")
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
        director_tool!(self, params, "resource_duplicate")
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
        director_tool!(self, params, "tilemap_set_cells")
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
        director_tool!(self, params, "tilemap_get_cells")
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
        director_tool!(self, params, "tilemap_clear")
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
        director_tool!(self, params, "gridmap_set_cells")
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
        director_tool!(self, params, "gridmap_get_cells")
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
        director_tool!(self, params, "gridmap_clear")
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
        director_tool!(self, params, "animation_create")
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
        director_tool!(self, params, "animation_add_track")
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
        director_tool!(self, params, "animation_read")
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
        director_tool!(self, params, "animation_remove_track")
    }

    #[tool(
        name = "physics_set_layers",
        description = "Set collision_layer and/or collision_mask bitmasks on a physics \
            node in a Godot scene. Works with any node that has collision properties \
            (PhysicsBody2D/3D, Area2D/3D, TileMapLayer, etc.). Always use this tool \
            instead of editing .tscn files directly."
    )]
    pub async fn physics_set_layers(
        &self,
        Parameters(params): Parameters<PhysicsSetLayersParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "physics_set_layers")
    }

    #[tool(
        name = "physics_set_layer_names",
        description = "Set human-readable names for physics, render, navigation, or \
            avoidance layers in project.godot. Layer numbers are 1-32. Valid layer types: \
            2d_physics, 3d_physics, 2d_render, 3d_render, 2d_navigation, 3d_navigation, \
            avoidance. Names appear in the editor's layer picker UI."
    )]
    pub async fn physics_set_layer_names(
        &self,
        Parameters(params): Parameters<PhysicsSetLayerNamesParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "physics_set_layer_names")
    }

    #[tool(
        name = "visual_shader_create",
        description = "Create a Godot VisualShader resource (.tres) with a node graph. \
            Define shader nodes and connections as JSON — the graph is built using \
            Godot's VisualShader API. Each node specifies a shader_function (vertex, \
            fragment, light, or particle functions) to target the correct processing \
            stage. Supports spatial (3D), canvas_item (2D), particles, sky, and fog \
            shader modes. Always use this instead of hand-writing shader .tres files."
    )]
    pub async fn visual_shader_create(
        &self,
        Parameters(params): Parameters<VisualShaderCreateParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "visual_shader_create")
    }

    #[tool(
        name = "batch",
        description = "Execute multiple Director operations in a single Godot process \
            invocation. Reduces cold-start overhead from N operations to 1. Operations \
            run in sequence. Use stop_on_error to control failure behavior. Cannot \
            contain nested batch calls."
    )]
    pub async fn batch(
        &self,
        Parameters(params): Parameters<BatchParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "batch")
    }

    #[tool(
        name = "scene_diff",
        description = "Compare two Godot scene files structurally. Returns lists of \
            added nodes, removed nodes, and changed properties. Supports git refs \
            (e.g., \"HEAD:scenes/player.tscn\") to compare against previous versions."
    )]
    pub async fn scene_diff(
        &self,
        Parameters(params): Parameters<SceneDiffParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "scene_diff")
    }

    #[tool(
        name = "uid_get",
        description = "Resolve the Godot UID for a file path. UIDs are stable identifiers \
            that persist across file renames and are used internally by Godot for resource \
            references."
    )]
    pub async fn uid_get(
        &self,
        Parameters(params): Parameters<UidGetParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "uid_get")
    }

    #[tool(
        name = "uid_update_project",
        description = "Scan project files and register any missing Godot UIDs. Run this \
            after creating files outside of Director to ensure the editor's UID cache \
            stays consistent."
    )]
    pub async fn uid_update_project(
        &self,
        Parameters(params): Parameters<UidUpdateProjectParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "uid_update_project")
    }

    #[tool(
        name = "export_mesh_library",
        description = "Export MeshInstance3D nodes from a Godot scene as a MeshLibrary \
            resource (.tres) for use with GridMap. Optionally filter which meshes to \
            include by node name. Collision shapes from CollisionShape3D children are \
            included automatically."
    )]
    pub async fn export_mesh_library(
        &self,
        Parameters(params): Parameters<ExportMeshLibraryParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "export_mesh_library")
    }

    #[tool(
        name = "signal_connect",
        description = "Connect a signal between two nodes in a Godot scene file (.tscn). \
            The connection is serialized into the scene and persists across loads. \
            Always use this tool instead of editing .tscn files directly — signal \
            connection blocks in .tscn are fragile and hand-editing will break them."
    )]
    pub async fn signal_connect(
        &self,
        Parameters(params): Parameters<SignalConnectParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "signal_connect")
    }

    #[tool(
        name = "signal_disconnect",
        description = "Remove a signal connection between two nodes in a Godot scene file (.tscn). \
            Always use this tool instead of editing .tscn files directly."
    )]
    pub async fn signal_disconnect(
        &self,
        Parameters(params): Parameters<SignalDisconnectParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "signal_disconnect")
    }

    #[tool(
        name = "signal_list",
        description = "List all signal connections in a Godot scene file (.tscn). Optionally \
            filter to connections involving a specific node. Returns source, signal name, \
            target, method, and flags for each connection."
    )]
    pub async fn signal_list(
        &self,
        Parameters(params): Parameters<SignalListParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "signal_list")
    }

    #[tool(
        name = "node_set_groups",
        description = "Add or remove a node from named groups in a Godot scene file (.tscn). \
            Groups are used for gameplay logic (e.g., 'enemies', 'interactable') and are \
            queryable at runtime via get_tree().get_nodes_in_group(). Always use this \
            tool instead of editing .tscn files directly."
    )]
    pub async fn node_set_groups(
        &self,
        Parameters(params): Parameters<NodeSetGroupsParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_set_groups")
    }

    #[tool(
        name = "node_set_script",
        description = "Attach or detach a GDScript (.gd) file to/from a node in a Godot \
            scene file (.tscn). The script must already exist on disk. Omit script_path \
            to detach. Always use this tool instead of editing .tscn files directly — \
            script references use internal resource IDs that are fragile to hand-edit."
    )]
    pub async fn node_set_script(
        &self,
        Parameters(params): Parameters<NodeSetScriptParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_set_script")
    }

    #[tool(
        name = "node_set_meta",
        description = "Set or remove metadata entries on a node in a Godot scene file (.tscn). \
            Metadata is arbitrary key-value data stored on nodes, useful for editor \
            annotations, gameplay tags, or tool configuration. Set a value to null to \
            remove that key. Always use this tool instead of editing .tscn files directly."
    )]
    pub async fn node_set_meta(
        &self,
        Parameters(params): Parameters<NodeSetMetaParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_set_meta")
    }

    #[tool(
        name = "node_find",
        description = "Search for nodes in a Godot scene file by class, group, name pattern, \
            or property. Multiple filters combine as AND. Returns matching node paths \
            and types. Use this to discover nodes without knowing the exact tree structure."
    )]
    pub async fn node_find(
        &self,
        Parameters(params): Parameters<NodeFindParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_find")
    }
}
