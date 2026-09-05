pub mod animation;
pub mod defaults;
pub mod editor_run;
pub mod engine_api;
pub mod feedback;
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
use editor_run::{EditorRunParams, EditorRunResponse};
use engine_api::{EngineApiParams, EngineApiResponse};
use gridmap::{GridMapClearParams, GridMapGetCellsParams, GridMapSetCellsParams};
use meta::{BatchParams, SceneDiffParams};
use node::{
    NodeAddParams, NodeFindParams, NodeRemoveParams, NodeReparentParams, NodeSetGroupsParams,
    NodeSetMetaParams, NodeSetPropertiesParams, NodeSetScriptParams,
};
use physics::{PhysicsSetLayerNamesParams, PhysicsSetLayersParams};
use project::{
    AutoloadAddParams, AutoloadRemoveParams, EditorStatusParams, ExportMeshLibraryParams,
    ProjectReloadParams, ProjectSettingsSetParams, UidGetParams, UidUpdateProjectParams,
};
use resource::{
    MaterialCreateParams, ResourceDuplicateParams, ResourceReadParams, ShapeCreateParams,
    StyleBoxCreateParams,
};
use scene::{
    SceneAddInstanceParams, SceneCreateParams, SceneListParams, SceneReadParams, SceneSaveParams,
};
use shader::VisualShaderCreateParams;
use signal::{SignalConnectParams, SignalDisconnectParams, SignalListParams};
use tilemap::{TileMapClearParams, TileMapGetCellsParams, TileMapSetCellsParams};

use stage_protocol::mcp_helpers::{deserialize_response, serialize_params, serialize_response};

use crate::responses::{
    AnimationAddTrackResponse, AnimationCreateResponse, AnimationReadResponse,
    AnimationRemoveTrackResponse, AutoloadAddResponse, AutoloadRemoveResponse, BatchResponse,
    EditorStatusRawResponse, EditorStatusResponse, ExportMeshLibraryResponse, GridMapClearResponse,
    GridMapGetCellsResponse, GridMapSetCellsResponse, NodeAddResponse, NodeFindResponse,
    NodeRemoveResponse, NodeReparentResponse, NodeSetGroupsResponse, NodeSetMetaResponse,
    NodeSetPropertiesResponse, NodeSetScriptResponse, PhysicsSetLayerNamesResponse,
    PhysicsSetLayersResponse, ProjectReloadResponse, ProjectSettingsSetResponse,
    ResourceCreateResponse, ResourceDuplicateResponse, ResourceReadResponse,
    SceneAddInstanceResponse, SceneCreateResponse, SceneDiffResponse, SceneListResponse,
    SceneReadResponse, SceneSaveResponse, ShapeCreateResponse, SignalConnectionResponse,
    SignalListResponse, TileMapClearResponse, TileMapGetCellsResponse, TileMapSetCellsResponse,
    UidGetResponse, UidUpdateProjectResponse, VisualShaderCreateResponse,
};

// ---------------------------------------------------------------------------
// Shared MCP helpers
// ---------------------------------------------------------------------------

macro_rules! director_tool {
    ($self:expr, $params:expr, $op:expr, $resp:ty) => {{
        let op_params = serialize_params(&$params)?;
        let data = run_operation(&$self.backend, &$params.project_path, $op, &op_params).await?;
        let typed: $resp = deserialize_response(data)?;
        serialize_response(&typed)
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
        description = "Read and manage persistent project-local human feedback without launching Godot. Status lists retained evidence and incomplete storage; retrieve returns matching selection, pointer, image and note without consuming it. Handle suppresses notices for all readers, while delete or cleanup explicitly removes storage."
    )]
    pub async fn feedback(
        &self,
        Parameters(params): Parameters<feedback::FeedbackParams>,
    ) -> Result<rmcp::model::CallToolResult, McpError> {
        theatre_feedback::mcp::execute(std::path::Path::new(&params.project_path), params.operation)
    }

    #[tool(
        name = "scene_save",
        description = "Save only the selected scene through Godot serialization. Open-scene edits remain unsaved until this explicit call. Preserves undo history and does not save unrelated scenes or external resources. The editor dirty marker may remain until a native editor save."
    )]
    pub async fn scene_save(
        &self,
        Parameters(params): Parameters<SceneSaveParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "scene_save", SceneSaveResponse)
    }

    #[tool(
        name = "scene_create",
        description = "Create a new Godot scene file (.tscn) with a specified root node type. \
            Prefer this Godot-backed operation for structural scene creation so engine types, \
            ownership, resource references, and serialization are handled natively."
    )]
    pub async fn scene_create(
        &self,
        Parameters(params): Parameters<SceneCreateParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "scene_create", SceneCreateResponse)
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
        director_tool!(self, params, "scene_read", SceneReadResponse)
    }

    #[tool(
        name = "node_add",
        description = "Add a node to a Godot scene file (.tscn). Optionally set initial \
            properties. Prefer this Godot-backed operation for structural scene edits so node \
            ownership, engine types, resource references, and serialization are handled natively."
    )]
    pub async fn node_add(
        &self,
        Parameters(params): Parameters<NodeAddParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_add", NodeAddResponse)
    }

    #[tool(
        name = "node_set_properties",
        description = "Set properties on a node in a Godot scene file (.tscn). Handles type \
            conversion automatically (Vector2, Vector3, Color, NodePath, resource paths). \
            Prefer this Godot-backed operation for structural property edits."
    )]
    pub async fn node_set_properties(
        &self,
        Parameters(params): Parameters<NodeSetPropertiesParams>,
    ) -> Result<String, McpError> {
        director_tool!(
            self,
            params,
            "node_set_properties",
            NodeSetPropertiesResponse
        )
    }

    #[tool(
        name = "node_remove",
        description = "Remove a node (and all its children) from a Godot scene file (.tscn). \
            Prefer this Godot-backed operation for structural scene edits."
    )]
    pub async fn node_remove(
        &self,
        Parameters(params): Parameters<NodeRemoveParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_remove", NodeRemoveResponse)
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
        director_tool!(self, params, "scene_list", SceneListResponse)
    }

    #[tool(
        name = "scene_add_instance",
        description = "Add a scene instance (reference) as a child node in another Godot scene. \
            The instanced scene is linked, not copied — changes to the source scene propagate. \
            Prefer this Godot-backed operation for structural scene edits."
    )]
    pub async fn scene_add_instance(
        &self,
        Parameters(params): Parameters<SceneAddInstanceParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "scene_add_instance", SceneAddInstanceResponse)
    }

    #[tool(
        name = "node_reparent",
        description = "Move a node to a new parent within the same Godot scene. Optionally \
            rename the node during the move. Keeps the local transform rather than global \
            position; native container layout still applies."
    )]
    pub async fn node_reparent(
        &self,
        Parameters(params): Parameters<NodeReparentParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_reparent", NodeReparentResponse)
    }

    #[tool(
        name = "engine_api",
        description = "Query the installed Godot engine's ClassDB for one class. Defaults to a \
            bounded summary; select properties, methods, signals, or enums, optionally by exact \
            member name. Focused results include inherited-member ownership, native type metadata, \
            relevant defaults, runtime engine version, and bounded pagination."
    )]
    pub async fn engine_api(
        &self,
        Parameters(params): Parameters<EngineApiParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "engine_api", EngineApiResponse)
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
        director_tool!(self, params, "resource_read", ResourceReadResponse)
    }

    #[tool(
        name = "material_create",
        description = "Create a Godot material resource (.tres). Supports StandardMaterial3D, \
            ORMMaterial3D, ShaderMaterial, CanvasItemMaterial, ParticleProcessMaterial, and \
            any ClassDB Material subclass. Prefer this operation for Godot-serialized materials."
    )]
    pub async fn material_create(
        &self,
        Parameters(params): Parameters<MaterialCreateParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "material_create", ResourceCreateResponse)
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
        director_tool!(self, params, "shape_create", ShapeCreateResponse)
    }

    #[tool(
        name = "style_box_create",
        description = "Create a Godot StyleBox resource (.tres) for UI theming. Supports \
            StyleBoxFlat, StyleBoxTexture, StyleBoxLine, and StyleBoxEmpty. Prefer this \
            operation for Godot-serialized StyleBox resources."
    )]
    pub async fn style_box_create(
        &self,
        Parameters(params): Parameters<StyleBoxCreateParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "style_box_create", ResourceCreateResponse)
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
        director_tool!(
            self,
            params,
            "resource_duplicate",
            ResourceDuplicateResponse
        )
    }

    #[tool(
        name = "tilemap_set_cells",
        description = "Set cells on a TileMapLayer node in a Godot scene. Each cell is placed by \
            grid coordinates, TileSet source ID, and atlas coordinates. The TileMapLayer must already \
            have a TileSet resource assigned. Prefer this Godot-backed structural edit."
    )]
    pub async fn tilemap_set_cells(
        &self,
        Parameters(params): Parameters<TileMapSetCellsParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "tilemap_set_cells", TileMapSetCellsResponse)
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
        director_tool!(self, params, "tilemap_get_cells", TileMapGetCellsResponse)
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
        director_tool!(self, params, "tilemap_clear", TileMapClearResponse)
    }

    #[tool(
        name = "gridmap_set_cells",
        description = "Set cells in a GridMap node in a Godot scene. Each cell is placed by 3D grid \
            position and MeshLibrary item index. The GridMap must already have a MeshLibrary resource \
            assigned. Prefer this Godot-backed structural edit."
    )]
    pub async fn gridmap_set_cells(
        &self,
        Parameters(params): Parameters<GridMapSetCellsParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "gridmap_set_cells", GridMapSetCellsResponse)
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
        director_tool!(self, params, "gridmap_get_cells", GridMapGetCellsResponse)
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
        director_tool!(self, params, "gridmap_clear", GridMapClearResponse)
    }

    #[tool(
        name = "animation_create",
        description = "Create a Godot Animation resource (.tres) with specified length and \
            loop mode. The animation starts empty — use animation_add_track to add tracks \
            and keyframes. Prefer this operation for Godot-serialized animations."
    )]
    pub async fn animation_create(
        &self,
        Parameters(params): Parameters<AnimationCreateParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "animation_create", AnimationCreateResponse)
    }

    #[tool(
        name = "animation_add_track",
        description = "Add a track with keyframes to a Godot Animation resource. Supports \
            value, position_3d, rotation_3d, scale_3d, blend_shape, method, and bezier \
            track types. Node paths are relative to the AnimationPlayer that will play this \
            animation. Prefer this Godot-backed structural edit."
    )]
    pub async fn animation_add_track(
        &self,
        Parameters(params): Parameters<AnimationAddTrackParams>,
    ) -> Result<String, McpError> {
        director_tool!(
            self,
            params,
            "animation_add_track",
            AnimationAddTrackResponse
        )
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
        director_tool!(self, params, "animation_read", AnimationReadResponse)
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
        director_tool!(
            self,
            params,
            "animation_remove_track",
            AnimationRemoveTrackResponse
        )
    }

    #[tool(
        name = "physics_set_layers",
        description = "Set collision_layer and/or collision_mask bitmasks on a physics \
            node in a Godot scene. Works with any node that has collision properties \
            (PhysicsBody2D/3D, Area2D/3D, TileMapLayer, etc.). Prefer this Godot-backed \
            structural edit."
    )]
    pub async fn physics_set_layers(
        &self,
        Parameters(params): Parameters<PhysicsSetLayersParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "physics_set_layers", PhysicsSetLayersResponse)
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
        director_tool!(
            self,
            params,
            "physics_set_layer_names",
            PhysicsSetLayerNamesResponse
        )
    }

    #[tool(
        name = "visual_shader_create",
        description = "Create a Godot VisualShader resource (.tres) with a node graph. \
            Define shader nodes and connections as JSON — the graph is built using \
            Godot's VisualShader API. Each node specifies a shader_function (vertex, \
            fragment, light, or particle functions) to target the correct processing \
            stage. Supports spatial (3D), canvas_item (2D), particles, sky, and fog \
            shader modes. Prefer this operation for Godot-serialized visual shaders; edit \
            shader source files directly with normal code tools."
    )]
    pub async fn visual_shader_create(
        &self,
        Parameters(params): Parameters<VisualShaderCreateParams>,
    ) -> Result<String, McpError> {
        director_tool!(
            self,
            params,
            "visual_shader_create",
            VisualShaderCreateResponse
        )
    }

    #[tool(
        name = "batch",
        description = "Execute multiple Director operations in a single Godot process \
            invocation. Reduces cold-start overhead from N operations to 1. Operations \
            run in sequence using the same editor/headless context as individual calls. \
            Each changed open-scene entry is undoable and unsaved until scene_save. \
            Use stop_on_error to control failure behavior. Errors retain earlier and partial \
            results and persistence; no rollback. Cannot contain nested batch calls."
    )]
    pub async fn batch(
        &self,
        Parameters(params): Parameters<BatchParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "batch", BatchResponse)
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
        director_tool!(self, params, "scene_diff", SceneDiffResponse)
    }

    #[tool(
        name = "autoload_add",
        description = "Add or update an autoload singleton in project.godot. Autoloads are \
            globally accessible singletons available in all GDScript files by name \
            (e.g. EventBus, GameState). Prefer this Godot-backed operation for the project \
            setting. Use project_reload after creating the script file and before calling this."
    )]
    pub async fn autoload_add(
        &self,
        Parameters(params): Parameters<AutoloadAddParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "autoload_add", AutoloadAddResponse)
    }

    #[tool(
        name = "autoload_remove",
        description = "Remove an autoload singleton from project.godot. The script file itself \
            is not deleted — only the autoload registration is removed."
    )]
    pub async fn autoload_remove(
        &self,
        Parameters(params): Parameters<AutoloadRemoveParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "autoload_remove", AutoloadRemoveResponse)
    }

    #[tool(
        name = "project_settings_set",
        description = "Set one or more project settings in project.godot. Keys use the format \
            \"section/key\" matching the .godot file structure. Common keys: \
            \"application/run/main_scene\" (main scene path), \
            \"application/config/name\" (project name), \
            \"display/window/size/viewport_width\", \
            \"display/window/size/viewport_height\". \
            Set a value to null to erase the key. Prefer this Godot-backed operation for \
            structural project-setting changes."
    )]
    pub async fn project_settings_set(
        &self,
        Parameters(params): Parameters<ProjectSettingsSetParams>,
    ) -> Result<String, McpError> {
        director_tool!(
            self,
            params,
            "project_settings_set",
            ProjectSettingsSetResponse
        )
    }

    #[tool(
        name = "project_reload",
        description = "Reload the project and validate all scripts. Call this after creating or \
            modifying .gd script files outside of Director (e.g. via Write tool). Returns \
            structured diagnostics — script parse errors, missing identifiers, broken \
            references — so you can fix issues before they cause failures in scene operations. \
            In headless mode this restarts the daemon; in editor mode it triggers a filesystem \
            rescan. Replaces the old filesystem_scan tool."
    )]
    pub async fn project_reload(
        &self,
        Parameters(params): Parameters<ProjectReloadParams>,
    ) -> Result<String, McpError> {
        use crate::diagnostics::parse_godot_stderr;
        use crate::oneshot::run_validation;

        let godot = resolve_godot_bin().map_err(McpError::from)?;
        let project = std::path::Path::new(&params.project_path);
        validate_project_path(project).map_err(McpError::from)?;

        // Kill stale daemon so next operation spawns fresh.
        self.backend.kill_daemon().await;

        // Run validation via one-shot (captures stderr).
        let op_params = serialize_params(&params)?;
        let validation = run_validation(&godot, project, "project_reload", &op_params)
            .await
            .map_err(McpError::from)?;

        // Parse stderr for Godot diagnostics.
        let diagnostics = parse_godot_stderr(&validation.stderr);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .cloned()
            .collect();
        let warnings: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == "warning")
            .cloned()
            .collect();

        // Extract GDScript data.
        let data = validation.result.into_data().map_err(McpError::from)?;
        let scripts_checked = data
            .get("scripts_checked")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let autoloads = data
            .get("autoloads")
            .cloned()
            .unwrap_or(serde_json::Value::Object(Default::default()));

        serialize_response(&ProjectReloadResponse {
            result: "ok".to_string(),
            scripts_checked,
            autoloads,
            errors,
            warnings,
        })
    }

    #[tool(
        name = "editor_run",
        description = "Control a saved scene through the verified Godot editor. Start and restart require scene_path; stop is idempotent and status is observational. Launch requests never implicitly save open work and never fall back to a headless process. The response reports native editor play state only; query Stage runtime_status separately for readiness and run_id."
    )]
    pub async fn editor_run(
        &self,
        Parameters(params): Parameters<EditorRunParams>,
    ) -> Result<String, McpError> {
        let response: EditorRunResponse = editor_run::run_editor(&self.backend, &params).await?;
        serialize_response(&response)
    }

    #[tool(
        name = "editor_status",
        description = "Get a snapshot of the Godot editor's current state — which scenes are \
            open, which is active, whether the game is running, registered autoloads, and \
            recent log output (errors, warnings, print statements from godot.log). \
            Use this to orient yourself before making changes, to check whether the editor \
            is running, or to see what errors exist. Reports Godot's actual project root and \
            answering process id. Works in headless mode too; editor_connected=false then \
            identifies a headless process, not an editor."
    )]
    pub async fn editor_status(
        &self,
        Parameters(params): Parameters<EditorStatusParams>,
    ) -> Result<String, McpError> {
        let op_params = serialize_params(&params)?;
        let data = run_operation(
            &self.backend,
            &params.project_path,
            "editor_status",
            &op_params,
        )
        .await?;

        // Deserialize the GDScript response.
        let raw: EditorStatusRawResponse = deserialize_response(data)?;

        // Parse recent_log lines into structured diagnostics.
        let log_text = raw.recent_log.join("\n");
        let diagnostics = crate::diagnostics::parse_godot_stderr(&log_text);
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .cloned()
            .collect();
        let warnings: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == "warning")
            .cloned()
            .collect();

        serialize_response(&EditorStatusResponse {
            project_path: raw.project_path,
            process_id: raw.process_id,
            editor_connected: raw.editor_connected,
            active_scene: raw.active_scene,
            open_scenes: raw.open_scenes,
            game_running: raw.game_running,
            autoloads: raw.autoloads,
            recent_log: raw.recent_log,
            errors,
            warnings,
        })
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
        director_tool!(self, params, "uid_get", UidGetResponse)
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
        director_tool!(self, params, "uid_update_project", UidUpdateProjectResponse)
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
        director_tool!(
            self,
            params,
            "export_mesh_library",
            ExportMeshLibraryResponse
        )
    }

    #[tool(
        name = "signal_connect",
        description = "Connect a signal between two nodes in a Godot scene file (.tscn). \
            The connection is serialized into the scene and persists across loads. Prefer this \
            Godot-backed operation for structural signal edits."
    )]
    pub async fn signal_connect(
        &self,
        Parameters(params): Parameters<SignalConnectParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "signal_connect", SignalConnectionResponse)
    }

    #[tool(
        name = "signal_disconnect",
        description = "Remove a signal connection between two nodes in a Godot scene file (.tscn). \
            Prefer this Godot-backed operation for structural signal edits."
    )]
    pub async fn signal_disconnect(
        &self,
        Parameters(params): Parameters<SignalDisconnectParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "signal_disconnect", SignalConnectionResponse)
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
        director_tool!(self, params, "signal_list", SignalListResponse)
    }

    #[tool(
        name = "node_set_groups",
        description = "Add or remove a node from named groups in a Godot scene file (.tscn). \
            Groups are used for gameplay logic (e.g., 'enemies', 'interactable') and are \
            queryable at runtime via get_tree().get_nodes_in_group(). Prefer this Godot-backed \
            structural edit."
    )]
    pub async fn node_set_groups(
        &self,
        Parameters(params): Parameters<NodeSetGroupsParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_set_groups", NodeSetGroupsResponse)
    }

    #[tool(
        name = "node_set_script",
        description = "Attach or detach a GDScript (.gd) file to/from a node in a Godot \
            scene file (.tscn). The script must already exist on disk. Omit script_path \
            to detach. Edit the GDScript source directly, then prefer this Godot-backed \
            operation for the structural scene attachment."
    )]
    pub async fn node_set_script(
        &self,
        Parameters(params): Parameters<NodeSetScriptParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_set_script", NodeSetScriptResponse)
    }

    #[tool(
        name = "node_set_meta",
        description = "Set or remove metadata entries on a node in a Godot scene file (.tscn). \
            Metadata is arbitrary key-value data stored on nodes, useful for editor \
            annotations, gameplay tags, or tool configuration. Set a value to null to \
            remove that key. Prefer this Godot-backed structural edit."
    )]
    pub async fn node_set_meta(
        &self,
        Parameters(params): Parameters<NodeSetMetaParams>,
    ) -> Result<String, McpError> {
        director_tool!(self, params, "node_set_meta", NodeSetMetaResponse)
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
        director_tool!(self, params, "node_find", NodeFindResponse)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// Assert that serializing a params struct with all optional fields absent
    /// produces no `null` values in the JSON. A `null` in the wire format breaks
    /// GDScript's `Dictionary.get(key, default)` — the key is present so the
    /// default is ignored and the typed assignment gets `Nil` instead.
    fn assert_no_nulls(json: &serde_json::Value) {
        match json {
            serde_json::Value::Null => panic!("unexpected null in serialized params"),
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    if v.is_null() {
                        panic!("null value for key '{k}' in serialized params");
                    }
                    assert_no_nulls(v);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    assert_no_nulls(v);
                }
            }
            _ => {}
        }
    }

    use super::*;

    #[test]
    fn scene_read_params_no_nulls_when_optional_absent() {
        let params = SceneReadParams {
            project_path: "/proj".into(),
            scene_path: "scenes/main.tscn".into(),
            depth: None,
            properties: true,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
        assert!(!json.as_object().unwrap().contains_key("depth"));
    }

    #[test]
    fn scene_list_params_no_nulls_when_optional_absent() {
        let params = SceneListParams {
            project_path: "/proj".into(),
            directory: None,
            pattern: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
    }

    #[test]
    fn node_add_params_no_nulls_when_optional_absent() {
        let params = NodeAddParams {
            project_path: "/proj".into(),
            scene_path: "s.tscn".into(),
            parent_path: ".".into(),
            node_type: "Node2D".into(),
            node_name: "Foo".into(),
            properties: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
    }

    #[test]
    fn node_find_params_no_nulls_when_optional_absent() {
        let params = NodeFindParams {
            project_path: "/proj".into(),
            scene_path: "s.tscn".into(),
            class_name: None,
            group: None,
            name_pattern: None,
            property: None,
            property_value: None,
            limit: 100,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
    }

    #[test]
    fn physics_set_layers_params_no_nulls_when_optional_absent() {
        let params = PhysicsSetLayersParams {
            project_path: "/proj".into(),
            scene_path: "s.tscn".into(),
            node_path: "Body".into(),
            collision_layer: None,
            collision_mask: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
    }

    #[test]
    fn project_reload_params_no_nulls() {
        let params = ProjectReloadParams {
            project_path: "/proj".into(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
    }

    #[test]
    fn editor_status_params_no_nulls() {
        let params = EditorStatusParams {
            project_path: "/proj".into(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
    }

    #[test]
    fn animation_add_track_params_no_nulls_when_optional_absent() {
        let params = AnimationAddTrackParams {
            project_path: "/proj".into(),
            resource_path: "anim.tres".into(),
            track_type: "value".into(),
            node_path: "Sprite2D:position".into(),
            keyframes: vec![],
            interpolation: None,
            update_mode: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_no_nulls(&json);
    }
}
