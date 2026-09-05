use serde::Deserialize;
use stage_core::cluster::ClusterStrategy;
use stage_core::config::{BearingFormat, SessionConfig};
use std::collections::HashMap;
use std::path::Path;

/// TOML file shape — maps to the documented stage.toml format.
/// All sections and fields are optional.
#[derive(Debug, Default, Deserialize)]
pub struct StageToml {
    pub connection: Option<ConnectionConfig>,
    pub tracking: Option<TrackingConfig>,
    pub dashcam: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ConnectionConfig {
    pub port: Option<u16>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TrackingConfig {
    pub static_patterns: Option<Vec<String>>,
    pub token_hard_cap: Option<u32>,
    pub state_properties: Option<HashMap<String, Vec<String>>>,
    pub cluster_by: Option<ClusterStrategy>,
    pub bearing_format: Option<BearingFormat>,
    pub expose_internals: Option<bool>,
    pub poll_interval: Option<u32>,
}

/// Load `stage.toml` from a directory. Returns `SessionConfig` with the
/// tracking-related fields applied. Returns default config if file not found.
pub fn load_toml_config(project_dir: &Path) -> SessionConfig {
    let toml_path = project_dir.join("stage.toml");
    match std::fs::read_to_string(&toml_path) {
        Ok(contents) => match toml::from_str::<StageToml>(&contents) {
            Ok(parsed) => {
                tracing::info!("Loaded config from {}", toml_path.display());
                toml_to_session_config(&parsed)
            }
            Err(e) => {
                tracing::warn!("Failed to parse {}: {}", toml_path.display(), e);
                SessionConfig::default()
            }
        },
        Err(_) => {
            tracing::debug!("No stage.toml found at {}", toml_path.display());
            SessionConfig::default()
        }
    }
}

/// Extract port from TOML config (separate from SessionConfig since port
/// is a connection concern, not a session config concern).
pub fn load_toml_port(project_dir: &Path) -> Option<u16> {
    let toml_path = project_dir.join("stage.toml");
    let contents = std::fs::read_to_string(toml_path).ok()?;
    let parsed: StageToml = toml::from_str(&contents).ok()?;
    parsed.connection.and_then(|c| c.port)
}

fn toml_to_session_config(toml: &StageToml) -> SessionConfig {
    let mut config = SessionConfig::default();
    if let Some(ref tracking) = toml.tracking {
        if let Some(ref v) = tracking.static_patterns {
            config.static_patterns = v.clone();
        }
        if let Some(ref v) = tracking.state_properties {
            config.state_properties = v.clone();
        }
        if let Some(v) = tracking.cluster_by {
            config.cluster_by = v;
        }
        if let Some(v) = tracking.bearing_format {
            config.bearing_format = v;
        }
        if let Some(v) = tracking.expose_internals {
            config.expose_internals = v;
        }
        if let Some(v) = tracking.poll_interval {
            config.poll_interval = v;
        }
        if let Some(v) = tracking.token_hard_cap {
            config.token_hard_cap = v;
        }
    }
    config
}

/// Read only the explicit recorder patch. Invalid recorder settings do not erase
/// valid connection/tracking settings or prevent the local tools from starting.
pub fn load_toml_dashcam(
    project_dir: &Path,
) -> Option<stage_protocol::dashcam::DashcamConfigPatch> {
    let path = project_dir.join("stage.toml");
    let text = std::fs::read_to_string(&path).ok()?;
    let parsed: StageToml = toml::from_str(&text).ok()?;
    let value = parsed.dashcam?;
    match value.try_into::<stage_protocol::dashcam::DashcamConfigPatch>() {
        Ok(patch) => Some(patch),
        Err(error) => {
            tracing::warn!(
                "Invalid [dashcam] in {}: {error}. Recorder settings were not applied.",
                path.display()
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn load_missing_toml() {
        let dir = TempDir::new().unwrap();
        let config = load_toml_config(dir.path());
        assert_eq!(config.token_hard_cap, 5000); // defaults
    }

    #[test]
    fn load_valid_toml() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("stage.toml");
        let mut f = std::fs::File::create(&toml_path).unwrap();
        writeln!(
            f,
            r#"
[connection]
port = 9078

[tracking]
static_patterns = ["walls/*", "terrain/*"]
token_hard_cap = 3000
cluster_by = "class"
"#
        )
        .unwrap();

        let config = load_toml_config(dir.path());
        assert_eq!(config.static_patterns, vec!["walls/*", "terrain/*"]);
        assert_eq!(config.token_hard_cap, 3000);
        assert_eq!(config.cluster_by, ClusterStrategy::Class);
    }

    #[test]
    fn load_toml_port_present() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("stage.toml");
        std::fs::write(&toml_path, "[connection]\nport = 9078\n").unwrap();
        assert_eq!(load_toml_port(dir.path()), Some(9078));
    }

    #[test]
    fn load_toml_port_absent() {
        let dir = TempDir::new().unwrap();
        assert_eq!(load_toml_port(dir.path()), None);
    }

    #[test]
    fn load_partial_toml_tracking_only() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("stage.toml");
        std::fs::write(&toml_path, "[tracking]\ntoken_hard_cap = 2000\n").unwrap();
        let config = load_toml_config(dir.path());
        assert_eq!(config.token_hard_cap, 2000);
        assert!(config.static_patterns.is_empty());
    }

    #[test]
    fn load_invalid_toml_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("stage.toml");
        std::fs::write(&toml_path, "not valid toml !!!{{").unwrap();
        let config = load_toml_config(dir.path());
        assert_eq!(config.token_hard_cap, 5000);
    }

    #[test]
    fn load_dashcam_config() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("stage.toml");
        let mut f = std::fs::File::create(&toml_path).unwrap();
        writeln!(
            f,
            r#"
[dashcam]
enabled = true
pre_window_system_sec = 45
pre_window_deliberate_sec = 90
post_window_system_sec = 15
post_window_deliberate_sec = 45
max_window_sec = 180
min_after_sec = 3
system_min_interval_sec = 5
byte_cap_mb = 512
capture_interval = 2
"#
        )
        .unwrap();

        let patch = load_toml_dashcam(dir.path()).unwrap();
        let config = patch
            .apply_to(&stage_protocol::dashcam::DashcamConfig::default())
            .unwrap();
        assert!(config.enabled);
        assert_eq!(config.pre_window_system_sec, 45);
        assert_eq!(config.pre_window_deliberate_sec, 90);
        assert_eq!(config.post_window_system_sec, 15);
        assert_eq!(config.post_window_deliberate_sec, 45);
        assert_eq!(config.max_window_sec, 180);
        assert_eq!(config.min_after_sec, 3);
        assert_eq!(config.system_min_interval_sec, 5);
        assert_eq!(config.byte_cap_mb, 512);
        assert_eq!(config.capture_interval, 2);
        assert!(
            patch.screenshot_enabled.is_none(),
            "omitted settings must not be pushed as defaults"
        );
    }

    #[test]
    fn dashcam_config_defaults_when_dashcam_section_absent() {
        let dir = TempDir::new().unwrap();
        assert!(load_toml_dashcam(dir.path()).is_none());
        std::fs::write(
            dir.path().join("stage.toml"),
            "[connection]\nport=9078\n[dashcam]\nunknown=true\n",
        )
        .unwrap();
        assert!(load_toml_dashcam(dir.path()).is_none());
        assert_eq!(load_toml_port(dir.path()), Some(9078));
    }
}
