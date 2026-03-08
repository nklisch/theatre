use serde::Deserialize;
use spectator_core::cluster::ClusterStrategy;
use spectator_core::config::{BearingFormat, SessionConfig};
use std::collections::HashMap;
use std::path::Path;

/// TOML file shape — maps to the documented spectator.toml format.
/// All sections and fields are optional.
#[derive(Debug, Default, Deserialize)]
pub struct SpectatorToml {
    pub connection: Option<ConnectionConfig>,
    pub tracking: Option<TrackingConfig>,
    pub recording: Option<RecordingConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RecordingConfig {
    pub dashcam_enabled: Option<bool>,
    pub dashcam_capture_interval: Option<u32>,
    pub dashcam_pre_window_system_sec: Option<u32>,
    pub dashcam_pre_window_deliberate_sec: Option<u32>,
    pub dashcam_post_window_system_sec: Option<u32>,
    pub dashcam_post_window_deliberate_sec: Option<u32>,
    pub dashcam_max_window_sec: Option<u32>,
    pub dashcam_min_after_sec: Option<u32>,
    pub dashcam_system_min_interval_sec: Option<u32>,
    pub dashcam_byte_cap_mb: Option<u32>,
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

/// Load `spectator.toml` from a directory. Returns `SessionConfig` with the
/// tracking-related fields applied. Returns default config if file not found.
pub fn load_toml_config(project_dir: &Path) -> SessionConfig {
    let toml_path = project_dir.join("spectator.toml");
    match std::fs::read_to_string(&toml_path) {
        Ok(contents) => match toml::from_str::<SpectatorToml>(&contents) {
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
            tracing::debug!("No spectator.toml found at {}", toml_path.display());
            SessionConfig::default()
        }
    }
}

/// Extract port from TOML config (separate from SessionConfig since port
/// is a connection concern, not a session config concern).
pub fn load_toml_port(project_dir: &Path) -> Option<u16> {
    let toml_path = project_dir.join("spectator.toml");
    let contents = std::fs::read_to_string(toml_path).ok()?;
    let parsed: SpectatorToml = toml::from_str(&contents).ok()?;
    parsed.connection.and_then(|c| c.port)
}

fn toml_to_session_config(toml: &SpectatorToml) -> SessionConfig {
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
    if let Some(ref rec) = toml.recording {
        if let Some(v) = rec.dashcam_enabled {
            config.dashcam_enabled = v;
        }
        if let Some(v) = rec.dashcam_capture_interval {
            config.dashcam_capture_interval = v;
        }
        if let Some(v) = rec.dashcam_pre_window_system_sec {
            config.dashcam_pre_window_system_sec = v;
        }
        if let Some(v) = rec.dashcam_pre_window_deliberate_sec {
            config.dashcam_pre_window_deliberate_sec = v;
        }
        if let Some(v) = rec.dashcam_post_window_system_sec {
            config.dashcam_post_window_system_sec = v;
        }
        if let Some(v) = rec.dashcam_post_window_deliberate_sec {
            config.dashcam_post_window_deliberate_sec = v;
        }
        if let Some(v) = rec.dashcam_max_window_sec {
            config.dashcam_max_window_sec = v;
        }
        if let Some(v) = rec.dashcam_min_after_sec {
            config.dashcam_min_after_sec = v;
        }
        if let Some(v) = rec.dashcam_system_min_interval_sec {
            config.dashcam_system_min_interval_sec = v;
        }
        if let Some(v) = rec.dashcam_byte_cap_mb {
            config.dashcam_byte_cap_mb = v;
        }
    }
    config
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
        let toml_path = dir.path().join("spectator.toml");
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
        let toml_path = dir.path().join("spectator.toml");
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
        let toml_path = dir.path().join("spectator.toml");
        std::fs::write(&toml_path, "[tracking]\ntoken_hard_cap = 2000\n").unwrap();
        let config = load_toml_config(dir.path());
        assert_eq!(config.token_hard_cap, 2000);
        assert!(config.static_patterns.is_empty());
    }

    #[test]
    fn load_invalid_toml_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("spectator.toml");
        std::fs::write(&toml_path, "not valid toml !!!{{").unwrap();
        let config = load_toml_config(dir.path());
        assert_eq!(config.token_hard_cap, 5000);
    }

    #[test]
    fn load_dashcam_recording_config() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("spectator.toml");
        let mut f = std::fs::File::create(&toml_path).unwrap();
        writeln!(
            f,
            r#"
[recording]
dashcam_enabled = true
dashcam_pre_window_system_sec = 45
dashcam_pre_window_deliberate_sec = 90
dashcam_post_window_system_sec = 15
dashcam_post_window_deliberate_sec = 45
dashcam_max_window_sec = 180
dashcam_min_after_sec = 3
dashcam_system_min_interval_sec = 5
dashcam_byte_cap_mb = 512
dashcam_capture_interval = 2
"#
        )
        .unwrap();

        let config = load_toml_config(dir.path());
        assert!(config.dashcam_enabled);
        assert_eq!(config.dashcam_pre_window_system_sec, 45);
        assert_eq!(config.dashcam_pre_window_deliberate_sec, 90);
        assert_eq!(config.dashcam_post_window_system_sec, 15);
        assert_eq!(config.dashcam_post_window_deliberate_sec, 45);
        assert_eq!(config.dashcam_max_window_sec, 180);
        assert_eq!(config.dashcam_min_after_sec, 3);
        assert_eq!(config.dashcam_system_min_interval_sec, 5);
        assert_eq!(config.dashcam_byte_cap_mb, 512);
        assert_eq!(config.dashcam_capture_interval, 2);
    }

    #[test]
    fn dashcam_config_defaults_when_recording_section_absent() {
        let dir = TempDir::new().unwrap();
        let config = load_toml_config(dir.path());
        assert!(config.dashcam_enabled);
        assert_eq!(config.dashcam_pre_window_system_sec, 30);
        assert_eq!(config.dashcam_pre_window_deliberate_sec, 60);
        assert_eq!(config.dashcam_post_window_system_sec, 10);
        assert_eq!(config.dashcam_post_window_deliberate_sec, 30);
        assert_eq!(config.dashcam_max_window_sec, 120);
        assert_eq!(config.dashcam_min_after_sec, 5);
        assert_eq!(config.dashcam_system_min_interval_sec, 2);
        assert_eq!(config.dashcam_byte_cap_mb, 1024);
    }
}
