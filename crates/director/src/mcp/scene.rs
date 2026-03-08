use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

fn default_true() -> bool {
    true
}
