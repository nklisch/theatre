use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `resource_read`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResourceReadParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Resource file to read (relative to project, e.g. "materials/ground.tres").
    pub resource_path: String,
}
