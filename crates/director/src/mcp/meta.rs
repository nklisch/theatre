use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::defaults::default_true;

/// A single operation within a batch.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct BatchOperation {
    /// The Director operation name (e.g. "scene_create", "node_add").
    pub operation: String,

    /// Parameters for this operation. Same format as calling the operation directly,
    /// but without project_path (inherited from the batch).
    pub params: serde_json::Map<String, serde_json::Value>,
}

/// Parameters for `batch`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct BatchParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Operations to execute in sequence.
    pub operations: Vec<BatchOperation>,

    /// If true (default), stop executing on first failure.
    /// If false, continue with remaining operations.
    #[serde(default = "default_true")]
    pub stop_on_error: bool,
}

/// Parameters for `scene_diff`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SceneDiffParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the first scene (relative to project, e.g. "scenes/player.tscn").
    /// Supports git ref syntax (e.g. "HEAD:scenes/player.tscn") to compare against
    /// previous versions.
    pub scene_a: String,

    /// Path to the second scene (relative to project).
    /// Supports git ref syntax (e.g. "HEAD:scenes/player.tscn").
    pub scene_b: String,
}
