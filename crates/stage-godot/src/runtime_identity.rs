use std::sync::OnceLock;

use godot::classes::{ProjectSettings, SceneTree};
use godot::prelude::*;
use stage_protocol::runtime::{RuntimeIdentity, RuntimeStatus};

/// Called only on the Godot main thread. All runtime surfaces share this
/// engine-owned identity, so a new TCP client never creates a new game run.
pub fn identity() -> &'static RuntimeIdentity {
    static IDENTITY: OnceLock<RuntimeIdentity> = OnceLock::new();
    IDENTITY.get_or_init(|| {
        let process_id = std::process::id();
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        RuntimeIdentity {
            project_path: ProjectSettings::singleton()
                .globalize_path("res://")
                .to_string(),
            process_id,
            run_id: format!("run_{process_id}_{started}"),
        }
    })
}

pub fn status(tree: Option<Gd<SceneTree>>) -> RuntimeStatus {
    let scene = tree.and_then(|tree| tree.get_current_scene());
    RuntimeStatus {
        identity: identity().clone(),
        ready: scene.as_ref().is_some_and(|scene| scene.is_node_ready()),
        current_scene: scene.map(|scene| scene.get_scene_file_path().to_string()),
    }
}
