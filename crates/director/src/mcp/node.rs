use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::defaults::default_root;

/// Parameters for `node_add`.
#[serde_with::skip_serializing_none]
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

/// Parameters for `node_reparent`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeReparentParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the node to move within the scene tree.
    pub node_path: String,

    /// Path to the new parent node within the scene tree.
    pub new_parent_path: String,

    /// Rename the node during reparent. Useful to avoid name collisions.
    #[serde(default)]
    pub new_name: Option<String>,
}

/// Parameters for `node_set_groups`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetGroupsParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree.
    pub node_path: String,

    /// Groups to add the node to.
    #[serde(default)]
    pub add: Option<Vec<String>>,

    /// Groups to remove the node from.
    #[serde(default)]
    pub remove: Option<Vec<String>>,
}

/// Parameters for `node_set_script`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetScriptParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree.
    pub node_path: String,

    /// Path to the .gd script file (relative to project, e.g., "scripts/player.gd").
    /// Omit or set to null to detach the current script.
    #[serde(default)]
    pub script_path: Option<String>,
}

/// Parameters for `node_set_meta`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeSetMetaParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the target node within the scene tree.
    pub node_path: String,

    /// Metadata entries to set. Keys are metadata names, values are the data.
    /// Set a value to null to remove that metadata key.
    pub meta: serde_json::Map<String, serde_json::Value>,
}

/// Parameters for `node_find`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct NodeFindParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Filter by Godot class name (supports inheritance via is_class()).
    #[serde(default)]
    pub class_name: Option<String>,

    /// Filter by group membership.
    #[serde(default)]
    pub group: Option<String>,

    /// Filter by node name pattern (supports * and ? wildcards).
    #[serde(default)]
    pub name_pattern: Option<String>,

    /// Filter: property must exist on the node.
    #[serde(default)]
    pub property: Option<String>,

    /// Filter: property must equal this value (requires `property` to also be set).
    #[serde(default)]
    pub property_value: Option<serde_json::Value>,

    /// Maximum number of results to return (default: 100).
    #[serde(default = "default_find_limit")]
    pub limit: u32,
}

fn default_find_limit() -> u32 {
    100
}
