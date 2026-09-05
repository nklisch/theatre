//! Typed response data structs for Director operations.
//!
//! These structs define the contract between GDScript addon responses and
//! the MCP output. The director_tool! macro deserializes GDScript's JSON
//! `data` field into one of these structs, catching shape mismatches early.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Files written and open scenes changed by this operation, not all editor dirty state.
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct Persistence {
    pub saved_paths: Vec<String>,
    pub unsaved_scene_paths: Vec<String>,
}

/// Selected-scene serialization retains undo and never flushes external resources.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneSaveResponse {
    pub scene_path: String,
    pub editor_dirty_marker_may_remain: bool,
    pub persistence: Persistence,
}

/// A batch entry retains partial data and context even when it fails.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct BatchEntryResponse {
    pub operation: String,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
    #[serde(default)]
    pub persistence: Persistence,
}

// ---------------------------------------------------------------------------
// Scene operations
// ---------------------------------------------------------------------------

/// Response for scene_create.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneCreateResponse {
    pub persistence: Persistence,
    pub path: String,
    pub root_type: String,
}

/// Response for scene_read.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneReadResponse {
    pub root: SceneNodeData,
}

/// A node in the scene tree returned by scene_read.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneNodeData {
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SceneNodeData>,
}

/// Response for scene_list.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneListResponse {
    pub scenes: Vec<SceneListEntry>,
}

/// A scene entry returned by scene_list.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneListEntry {
    pub path: String,
    pub root_type: String,
    pub node_count: u32,
}

/// Response for scene_add_instance.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneAddInstanceResponse {
    pub persistence: Persistence,
    pub node_path: String,
    pub instance_scene: String,
}

// ---------------------------------------------------------------------------
// Node operations
// ---------------------------------------------------------------------------

/// Response for node_add.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeAddResponse {
    pub persistence: Persistence,
    pub node_path: String,
    #[serde(rename = "type")]
    pub node_type: String,
}

/// Response for node_set_properties.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeSetPropertiesResponse {
    pub persistence: Persistence,
    pub node_path: String,
    pub properties_set: Vec<String>,
}

/// Response for node_remove.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeRemoveResponse {
    pub persistence: Persistence,
    /// The path of the removed node.
    pub removed: String,
    /// Number of child nodes also removed.
    pub children_removed: u32,
}

/// Response for node_reparent.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeReparentResponse {
    pub persistence: Persistence,
    pub old_path: String,
    pub new_path: String,
}

/// Response for node_set_groups.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeSetGroupsResponse {
    pub persistence: Persistence,
    pub node_path: String,
    pub groups: Vec<String>,
}

/// Response for node_set_script.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeSetScriptResponse {
    pub persistence: Persistence,
    pub node_path: String,
    pub script_path: Option<String>,
}

/// Response for node_set_meta.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeSetMetaResponse {
    pub persistence: Persistence,
    pub node_path: String,
    pub meta_keys: Vec<String>,
}

/// Response for node_find.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeFindResponse {
    pub results: Vec<NodeFindEntry>,
}

/// A node entry returned by node_find.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeFindEntry {
    pub node_path: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Resource operations
// ---------------------------------------------------------------------------

/// Response for resource_read.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResourceReadResponse {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub path: String,
    pub properties: serde_json::Value,
    /// Present when reading a .tscn file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Response for material_create and style_box_create.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResourceCreateResponse {
    pub persistence: Persistence,
    pub path: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

/// Response for shape_create.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShapeCreateResponse {
    pub persistence: Persistence,
    pub shape_type: String,
    /// Set when a save_path was provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_to: Option<String>,
    /// Set when a scene_path + node_path was provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_to: Option<String>,
}

/// Response for resource_duplicate.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResourceDuplicateResponse {
    pub persistence: Persistence,
    pub path: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub overrides_applied: Vec<String>,
}

// ---------------------------------------------------------------------------
// TileMap operations
// ---------------------------------------------------------------------------

/// Response for tilemap_set_cells.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TileMapSetCellsResponse {
    pub persistence: Persistence,
    pub cells_set: u32,
    pub node_path: String,
}

/// Response for tilemap_get_cells.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TileMapGetCellsResponse {
    pub cells: Vec<serde_json::Value>,
    pub cell_count: u32,

    pub used_rect: serde_json::Value,
}

/// Response for tilemap_clear.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TileMapClearResponse {
    pub persistence: Persistence,
    pub cells_cleared: u32,
    pub node_path: String,
}

// ---------------------------------------------------------------------------
// GridMap operations
// ---------------------------------------------------------------------------

/// Response for gridmap_set_cells.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GridMapSetCellsResponse {
    pub persistence: Persistence,
    pub cells_set: u32,
    pub node_path: String,
}

/// Response for gridmap_get_cells.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GridMapGetCellsResponse {
    pub cells: Vec<serde_json::Value>,
    pub cell_count: u32,
}

/// Response for gridmap_clear.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GridMapClearResponse {
    pub persistence: Persistence,
    pub cells_cleared: u32,
    pub node_path: String,
}

// ---------------------------------------------------------------------------
// Animation operations
// ---------------------------------------------------------------------------

/// Response for animation_create.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnimationCreateResponse {
    pub persistence: Persistence,
    pub path: String,
    pub length: f64,
    pub loop_mode: String,
}

/// Response for animation_add_track.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnimationAddTrackResponse {
    pub persistence: Persistence,
    pub track_index: u32,
    pub keyframe_count: u32,
}

/// Response for animation_read.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnimationReadResponse {
    pub path: String,
    pub length: f64,
    pub loop_mode: String,
    pub step: f64,

    pub tracks: Vec<serde_json::Value>,
}

/// Response for animation_remove_track.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnimationRemoveTrackResponse {
    pub persistence: Persistence,
    pub tracks_removed: u32,
}

// ---------------------------------------------------------------------------
// Physics operations
// ---------------------------------------------------------------------------

/// Response for physics_set_layers.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PhysicsSetLayersResponse {
    pub persistence: Persistence,
    pub node_path: String,
    pub collision_layer: u32,
    pub collision_mask: u32,
}

/// Response for physics_set_layer_names.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PhysicsSetLayerNamesResponse {
    pub persistence: Persistence,
    pub layer_type: String,
    pub layers_set: u32,
}

// ---------------------------------------------------------------------------
// Shader operations
// ---------------------------------------------------------------------------

/// Response for visual_shader_create.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct VisualShaderCreateResponse {
    pub persistence: Persistence,
    pub path: String,
    pub node_count: u32,
    pub connection_count: u32,
}

// ---------------------------------------------------------------------------
// Signal operations
// ---------------------------------------------------------------------------

/// Response for signal_connect and signal_disconnect.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignalConnectionResponse {
    pub persistence: Persistence,
    pub source_path: String,
    pub signal_name: String,
    pub target_path: String,
    pub method_name: String,
}

/// Response for signal_list.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignalListResponse {
    pub connections: Vec<SignalConnectionEntry>,
}

/// A signal connection entry returned by signal_list.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignalConnectionEntry {
    pub source_path: String,
    pub signal_name: String,
    pub target_path: String,
    pub method_name: String,
    pub flags: u32,
    #[serde(default)]
    pub binds: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Meta operations
// ---------------------------------------------------------------------------

/// Response for batch.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct BatchResponse {
    pub persistence: Persistence,
    pub results: Vec<BatchEntryResponse>,
    pub completed: u32,
    pub failed: u32,
}

/// Response for scene_diff.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SceneDiffResponse {
    pub added: Vec<serde_json::Value>,

    pub removed: Vec<serde_json::Value>,

    pub moved: Vec<serde_json::Value>,

    pub changed: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Project operations
// ---------------------------------------------------------------------------

/// Response for autoload_add.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AutoloadAddResponse {
    pub persistence: Persistence,
    pub name: String,
    pub script_path: String,
    pub enabled: bool,
}

/// Response for autoload_remove.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AutoloadRemoveResponse {
    pub persistence: Persistence,
    pub name: String,
}

/// Response for project_settings_set.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectSettingsSetResponse {
    pub persistence: Persistence,
    pub keys_set: Vec<String>,
}

/// Response for project_reload.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectReloadResponse {
    pub result: String,
    pub scripts_checked: u32,
    pub autoloads: serde_json::Value,
    pub errors: Vec<crate::diagnostics::GodotDiagnostic>,
    pub warnings: Vec<crate::diagnostics::GodotDiagnostic>,
}

/// Response for editor_status.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditorStatusResponse {
    /// Actual project root reported by Godot.
    pub project_path: String,
    /// Process answering this call; an editor only when editor_connected is true.
    pub process_id: u32,
    /// Whether the Godot editor is running and connected.
    pub editor_connected: bool,

    /// Currently active scene in the editor (project-relative path, empty if none).
    pub active_scene: String,

    /// All scenes currently open in editor tabs.
    pub open_scenes: Vec<String>,

    /// Whether the game is currently running (F5 pressed).
    pub game_running: bool,

    /// Registered autoload singletons (name → script path).
    pub autoloads: serde_json::Value,

    /// Recent lines from godot.log (last 50 lines, includes errors/warnings/print output).
    pub recent_log: Vec<String>,

    /// Structured errors parsed from the log.
    pub errors: Vec<crate::diagnostics::GodotDiagnostic>,

    /// Structured warnings parsed from the log.
    pub warnings: Vec<crate::diagnostics::GodotDiagnostic>,
}

/// Raw GDScript response for editor_status — before Rust-side log parsing.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct EditorStatusRawResponse {
    pub project_path: String,
    pub process_id: u32,
    pub editor_connected: bool,
    pub active_scene: String,
    pub open_scenes: Vec<String>,
    pub game_running: bool,
    pub autoloads: serde_json::Value,
    pub recent_log: Vec<String>,
}

/// Response for uid_get.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UidGetResponse {
    pub file_path: String,
    pub uid: String,
}

/// Response for uid_update_project.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UidUpdateProjectResponse {
    pub persistence: Persistence,
    pub files_scanned: u32,
    pub uids_registered: u32,
}

/// Response for export_mesh_library.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExportMeshLibraryResponse {
    pub persistence: Persistence,
    pub path: String,
    pub items_exported: u32,
}
