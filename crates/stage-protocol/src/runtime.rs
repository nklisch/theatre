use serde::{Deserialize, Serialize};

/// Engine-owned identity of one running game, independent of client connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RuntimeIdentity {
    /// Actual project root, reported by Godot rather than inferred from a port.
    pub project_path: String,
    pub process_id: u32,
    /// Stable across client reconnects; changes when the game process restarts.
    pub run_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RuntimeStatusParams {}

/// Current engine state, queried on demand rather than cached at handshake time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RuntimeStatus {
    pub identity: RuntimeIdentity,
    /// A current scene exists and has completed its ready notification.
    pub ready: bool,
    /// Godot resource path of the current scene, if one is loaded.
    pub current_scene: Option<String>,
}
