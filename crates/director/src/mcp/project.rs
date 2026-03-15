use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::defaults::default_true;

/// Parameters for `autoload_add`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AutoloadAddParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Autoload singleton name as it will appear in code (e.g. "EventBus", "GameState").
    pub name: String,

    /// Script path relative to the project root (e.g. "autoload/event_bus.gd").
    pub script_path: String,

    /// Whether the autoload is active. Default: true.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Parameters for `autoload_remove`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AutoloadRemoveParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Autoload singleton name to remove.
    pub name: String,
}

/// Parameters for `project_settings_set`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ProjectSettingsSetParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Map of setting keys to values. Keys use the format "section/key" matching
    /// the project.godot file structure, e.g.:
    /// - "application/run/main_scene" → sets the main scene path
    /// - "application/config/name"    → sets the project display name
    /// - "display/window/size/viewport_width" → sets window width
    /// Set a value to null to erase the key.
    pub settings: serde_json::Map<String, serde_json::Value>,
}

/// Parameters for `project_reload`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ProjectReloadParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,
}

/// Parameters for `editor_status`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EditorStatusParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,
}

/// Parameters for `uid_get`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UidGetParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// File path relative to project (e.g. "scenes/player.tscn").
    pub file_path: String,
}

/// Parameters for `uid_update_project`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UidUpdateProjectParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Subdirectory to scan (relative to project). Default: scan entire project.
    #[serde(default)]
    pub directory: Option<String>,
}

/// Parameters for `export_mesh_library`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ExportMeshLibraryParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Source scene containing MeshInstance3D nodes (relative to project).
    pub scene_path: String,

    /// Output path for the MeshLibrary .tres file (relative to project).
    pub output_path: String,

    /// Optional list of MeshInstance3D node names to include.
    /// If omitted, all MeshInstance3D children of the scene root are included.
    #[serde(default)]
    pub items: Option<Vec<String>>,
}
