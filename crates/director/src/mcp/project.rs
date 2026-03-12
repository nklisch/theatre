use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `uid_get`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UidGetParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// File path relative to project (e.g. "scenes/player.tscn").
    pub file_path: String,
}

/// Parameters for `uid_update_project`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UidUpdateProjectParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Subdirectory to scan (relative to project). Default: scan entire project.
    #[serde(default)]
    pub directory: Option<String>,
}

/// Parameters for `export_mesh_library`.
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
