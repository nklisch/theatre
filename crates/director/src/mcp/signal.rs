use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `signal_connect`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SignalConnectParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the node emitting the signal (relative to scene root, e.g., "Button1").
    pub source_path: String,

    /// Signal name (e.g., "pressed", "body_entered").
    pub signal_name: String,

    /// Path to the node receiving the signal.
    pub target_path: String,

    /// Method name to call on the target node.
    pub method_name: String,

    /// Optional extra arguments to pass to the method.
    #[serde(default)]
    pub binds: Option<Vec<serde_json::Value>>,

    /// Optional connection flags bitmask (CONNECT_DEFERRED=1, CONNECT_PERSIST=2,
    /// CONNECT_ONE_SHOT=4). CONNECT_PERSIST is added automatically for scene
    /// serialization.
    #[serde(default)]
    pub flags: Option<u32>,
}

/// Parameters for `signal_disconnect`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SignalDisconnectParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Path to the node emitting the signal.
    pub source_path: String,

    /// Signal name to disconnect.
    pub signal_name: String,

    /// Path to the node that was receiving the signal.
    pub target_path: String,

    /// Method name that was connected.
    pub method_name: String,
}

/// Parameters for `signal_list`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SignalListParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the scene file relative to the project root.
    pub scene_path: String,

    /// Optional: filter connections involving this node (as source or target).
    #[serde(default)]
    pub node_path: Option<String>,
}
