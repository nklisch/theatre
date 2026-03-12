use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `physics_set_layers`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PhysicsSetLayersParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file to modify (relative to project, e.g. "scenes/player.tscn").
    pub scene_path: String,

    /// Path to the node within the scene tree (e.g. "Player/CollisionShape2D").
    pub node_path: String,

    /// Collision layer bitmask (32-bit unsigned integer). Determines which
    /// physics layers this object occupies. Omit to leave unchanged.
    #[serde(default)]
    pub collision_layer: Option<u32>,

    /// Collision mask bitmask (32-bit unsigned integer). Determines which
    /// physics layers this object scans/detects. Omit to leave unchanged.
    #[serde(default)]
    pub collision_mask: Option<u32>,
}

/// Parameters for `physics_set_layer_names`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PhysicsSetLayerNamesParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Which layer category to configure.
    /// Valid values: "2d_physics", "3d_physics", "2d_render", "3d_render",
    /// "2d_navigation", "3d_navigation", "avoidance".
    pub layer_type: String,

    /// Map of layer number (1-32) to human-readable name.
    /// Example: {"1": "player", "2": "enemies", "5": "projectiles"}.
    /// Layers not included are left unchanged.
    pub layers: serde_json::Map<String, serde_json::Value>,
}
