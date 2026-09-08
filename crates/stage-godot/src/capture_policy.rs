use godot::prelude::*;
use stage_protocol::capture::{LaunchOptions, parse_project_config, resolve};

/// Pure configuration boundary. Creating this value starts no engine services.
#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct StageCapturePolicy {
    base: Base<RefCounted>,
}

#[godot_api]
impl StageCapturePolicy {
    #[func]
    pub fn resolve_runtime(resolved: GString, arguments: GString) -> GString {
        use stage_protocol::capture::{LaunchConfig, from_godot_json, resolve_from};
        let result = (|| -> Result<LaunchConfig, String> {
            let config: LaunchConfig =
                from_godot_json(&resolved.to_string()).map_err(|e| e.to_string())?;
            let arguments: LaunchOptions =
                from_godot_json(&arguments.to_string()).map_err(|e| e.to_string())?;
            resolve_from(config, &[("arguments", arguments)])
        })();
        let value = match result {
            Ok(config) => serde_json::json!({"ok":true,"config":config}),
            Err(error) => serde_json::json!({"ok":false,"error":error}),
        };
        GString::from(&value.to_string())
    }

    #[func]
    pub fn configure_play(current: GString, patch: GString) -> GString {
        use stage_protocol::capture::{LaunchConfig, PhaseOptions, from_godot_json};
        let result = (|| -> Result<LaunchConfig, String> {
            let current: LaunchConfig =
                from_godot_json(&current.to_string()).map_err(|e| e.to_string())?;
            let patch: PhaseOptions =
                from_godot_json(&patch.to_string()).map_err(|e| e.to_string())?;
            current.configure_play(&patch)
        })();
        let value = match result {
            Ok(config) => serde_json::json!({"ok":true, "config":config}),
            Err(error) => serde_json::json!({"ok":false, "error":error}),
        };
        GString::from(&value.to_string())
    }

    #[func]
    pub fn resolve_launch(
        project: GString,
        user: GString,
        local: GString,
        explicit: GString,
    ) -> GString {
        let result = (|| {
            let mut layers = Vec::new();
            for (source, contents) in [("project", project), ("user", user), ("local", local)] {
                let patch = parse_project_config(&contents.to_string())
                    .map_err(|error| format!("Invalid {source} launch configuration: {error}"))?;
                layers.push((source, patch));
            }
            let patch: LaunchOptions =
                stage_protocol::capture::from_godot_json(&explicit.to_string())
                    .map_err(|error| format!("Invalid launch options: {error}"))?;
            layers.push(("launch", patch));
            resolve(&layers)
        })();
        let value = match result {
            Ok(config) => serde_json::json!({"ok":true, "config":config}),
            Err(error) => serde_json::json!({"ok":false, "error":error}),
        };
        GString::from(&value.to_string())
    }
}
