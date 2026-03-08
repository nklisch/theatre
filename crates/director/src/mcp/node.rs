use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `node_add`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeAddParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the parent node within the scene tree (e.g., "." for root, "Player" for
    /// a child named Player). Default: root node (".").
    #[serde(default = "default_root")]
    pub parent_path: String,

    /// The Godot class name for the new node (e.g., "Sprite2D", "CollisionShape2D").
    pub node_type: String,

    /// Name for the new node.
    pub node_name: String,

    /// Optional initial properties to set on the node after creation.
    /// Keys are property names, values are JSON representations of the property values.
    /// Type conversion is handled automatically (e.g., {"x": 100, "y": 200} for Vector2).
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Parameters for `node_set_properties`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetPropertiesParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree (e.g., "Player/Sprite2D").
    pub node_path: String,

    /// Properties to set. Keys are property names, values are JSON representations.
    /// Type conversion is automatic: Vector2 from {"x":1,"y":2}, Color from "#ff0000"
    /// or {"r":1,"g":0,"b":0}, NodePath from string, resources from "res://" paths.
    pub properties: serde_json::Map<String, serde_json::Value>,
}

/// Parameters for `node_remove`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeRemoveParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the node to remove within the scene tree.
    /// All children of this node are also removed.
    pub node_path: String,
}

fn default_root() -> String {
    ".".to_string()
}
