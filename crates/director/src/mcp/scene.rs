use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::defaults::{default_root, default_true};

/// Parameters for `scene_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SceneCreateParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Path to the scene file relative to the project root (e.g., "scenes/player.tscn").
    pub scene_path: String,

    /// The Godot class name for the root node (e.g., "Node2D", "Node3D", "Control").
    pub root_type: String,
}

/// Parameters for `scene_read`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SceneReadParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Path to the scene file relative to the project root (e.g., "scenes/player.tscn").
    pub scene_path: String,

    /// Maximum tree depth to include (default: unlimited).
    #[serde(default)]
    pub depth: Option<u32>,

    /// Whether to include node properties in the output (default: true).
    #[serde(default = "default_true")]
    pub properties: bool,
}

/// Parameters for `scene_list`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SceneListParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,

    /// Subdirectory to list (relative to project root). Lists entire project if omitted.
    #[serde(default)]
    pub directory: Option<String>,

    /// Glob pattern to filter scene paths (e.g., "scenes/**/*.tscn").
    /// Uses Godot's String.match() which supports * and ? wildcards.
    /// Omitting returns all scenes (backward-compatible).
    #[serde(default)]
    pub pattern: Option<String>,
}

/// Parameters for `scene_add_instance`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SceneAddInstanceParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file to modify (relative to project, e.g. "scenes/level.tscn").
    pub scene_path: String,

    /// Scene to instance (relative to project, e.g. "scenes/player.tscn").
    pub instance_scene: String,

    /// Parent node path within the target scene (default: root ".").
    #[serde(default = "default_root")]
    pub parent_path: String,

    /// Override the instance root's name. Uses the instanced scene's root name if omitted.
    #[serde(default)]
    pub node_name: Option<String>,
}
