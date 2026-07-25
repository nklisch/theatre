#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cluster::ClusterStrategy;

/// Bearing output format preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BearingFormat {
    Cardinal,
    Degrees,
    #[default]
    Both,
}

/// Active session configuration.
///
/// Three sources with precedence: spatial_config (session) > stage.toml (project) > Project Settings (machine).
/// This struct holds the merged effective config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SessionConfig {
    /// Glob patterns for static node classification.
    /// Nodes matching these patterns are treated as static regardless of class.
    #[serde(default)]
    pub static_patterns: Vec<String>,

    /// Properties to include in state output, keyed by group name or class name.
    /// Key "*" applies to all nodes. Empty map = include all exported vars (default).
    #[serde(default)]
    pub state_properties: HashMap<String, Vec<String>>,

    /// How to cluster nodes in summary views.
    #[serde(default)]
    pub cluster_by: ClusterStrategy,

    /// Bearing format preference.
    #[serde(default)]
    pub bearing_format: BearingFormat,

    /// Whether to include non-exported (internal) variables in state output.
    #[serde(default)]
    pub expose_internals: bool,

    /// Physics tick polling rate (every N physics frames). 1 = every frame.
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u32,

    /// Hard cap on token budget for any single response.
    #[serde(default = "default_token_hard_cap")]
    pub token_hard_cap: u32,

    // --- Dashcam config (M11) ---
    #[serde(default = "default_dashcam_enabled")]
    pub dashcam_enabled: bool,

    #[serde(default = "default_dashcam_capture_interval")]
    pub dashcam_capture_interval: u32,

    #[serde(default = "default_pre_window_system")]
    pub dashcam_pre_window_system_sec: u32,

    #[serde(default = "default_pre_window_deliberate")]
    pub dashcam_pre_window_deliberate_sec: u32,

    #[serde(default = "default_post_window_system")]
    pub dashcam_post_window_system_sec: u32,

    #[serde(default = "default_post_window_deliberate")]
    pub dashcam_post_window_deliberate_sec: u32,

    #[serde(default = "default_max_window")]
    pub dashcam_max_window_sec: u32,

    #[serde(default = "default_min_after")]
    pub dashcam_min_after_sec: u32,

    #[serde(default = "default_system_min_interval")]
    pub dashcam_system_min_interval_sec: u32,

    #[serde(default = "default_byte_cap_mb")]
    pub dashcam_byte_cap_mb: u32,

    /// Screenshot cadence in physics frames (the visual-storyboards capture clock).
    #[serde(default = "default_screenshot_interval_frames")]
    pub dashcam_screenshot_interval_frames: u32,
    #[serde(default = "default_screenshot_quality")]
    pub dashcam_screenshot_quality: f32,
    #[serde(default = "default_screenshot_max_dimension")]
    pub dashcam_screenshot_max_dimension: u32,
    #[serde(default = "default_screenshot_byte_cap_mb")]
    pub dashcam_screenshot_byte_cap_mb: u32,
    #[serde(default = "default_dense_burst_enabled")]
    pub dashcam_dense_burst_enabled: bool,
    #[serde(default = "default_dense_burst_interval_frames")]
    pub dashcam_dense_burst_interval_frames: u32,
    #[serde(default = "default_dense_burst_duration_sec")]
    pub dashcam_dense_burst_duration_sec: u32,
    #[serde(default = "default_anomaly_enabled")]
    pub dashcam_anomaly_enabled: bool,
    #[serde(default = "default_anomaly_min")]
    pub dashcam_anomaly_min_proportion: f64,
    #[serde(default = "default_anomaly_relative")]
    pub dashcam_anomaly_relative_factor: f64,
    #[serde(default = "default_anomaly_sustained")]
    pub dashcam_anomaly_sustained_frames: u32,
    #[serde(default = "default_anomaly_cooldown")]
    pub dashcam_anomaly_cooldown_sec: u32,
    #[serde(default = "default_anomaly_noise")]
    pub dashcam_anomaly_noise_floor: u8,

    /// True when the project's stage.toml actually contains a `[dashcam]`
    /// section. The post-handshake config push only happens when explicit —
    /// otherwise every one-shot CLI connection would clobber runtime config
    /// applied via `clips(config)` with built-in defaults.
    #[serde(skip)]
    pub dashcam_explicit: bool,
}

fn default_poll_interval() -> u32 {
    1
}
fn default_token_hard_cap() -> u32 {
    5000
}
fn default_dashcam_enabled() -> bool {
    true
}
fn default_dashcam_capture_interval() -> u32 {
    1
}
fn default_pre_window_system() -> u32 {
    30
}
fn default_pre_window_deliberate() -> u32 {
    60
}
fn default_post_window_system() -> u32 {
    10
}
fn default_post_window_deliberate() -> u32 {
    30
}
fn default_max_window() -> u32 {
    120
}
fn default_min_after() -> u32 {
    5
}
fn default_system_min_interval() -> u32 {
    2
}
fn default_byte_cap_mb() -> u32 {
    1024
}
fn default_screenshot_interval_frames() -> u32 {
    4
}
fn default_screenshot_quality() -> f32 {
    0.65
}
fn default_screenshot_max_dimension() -> u32 {
    480
}
fn default_screenshot_byte_cap_mb() -> u32 {
    32
}
fn default_dense_burst_enabled() -> bool {
    false
}
fn default_dense_burst_interval_frames() -> u32 {
    2
}
fn default_dense_burst_duration_sec() -> u32 {
    15
}
fn default_anomaly_enabled() -> bool {
    true
}
fn default_anomaly_min() -> f64 {
    0.30
}
fn default_anomaly_relative() -> f64 {
    4.0
}
fn default_anomaly_sustained() -> u32 {
    4
}
fn default_anomaly_cooldown() -> u32 {
    30
}
fn default_anomaly_noise() -> u8 {
    24
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            static_patterns: Vec::new(),
            state_properties: HashMap::new(),
            cluster_by: ClusterStrategy::default(),
            bearing_format: BearingFormat::default(),
            expose_internals: false,
            poll_interval: default_poll_interval(),
            token_hard_cap: default_token_hard_cap(),
            dashcam_enabled: default_dashcam_enabled(),
            dashcam_capture_interval: default_dashcam_capture_interval(),
            dashcam_pre_window_system_sec: default_pre_window_system(),
            dashcam_pre_window_deliberate_sec: default_pre_window_deliberate(),
            dashcam_post_window_system_sec: default_post_window_system(),
            dashcam_post_window_deliberate_sec: default_post_window_deliberate(),
            dashcam_max_window_sec: default_max_window(),
            dashcam_min_after_sec: default_min_after(),
            dashcam_system_min_interval_sec: default_system_min_interval(),
            dashcam_byte_cap_mb: default_byte_cap_mb(),
            dashcam_screenshot_interval_frames: default_screenshot_interval_frames(),
            dashcam_screenshot_quality: default_screenshot_quality(),
            dashcam_screenshot_max_dimension: default_screenshot_max_dimension(),
            dashcam_screenshot_byte_cap_mb: default_screenshot_byte_cap_mb(),
            dashcam_dense_burst_enabled: default_dense_burst_enabled(),
            dashcam_dense_burst_interval_frames: default_dense_burst_interval_frames(),
            dashcam_dense_burst_duration_sec: default_dense_burst_duration_sec(),
            dashcam_anomaly_enabled: default_anomaly_enabled(),
            dashcam_anomaly_min_proportion: default_anomaly_min(),
            dashcam_anomaly_relative_factor: default_anomaly_relative(),
            dashcam_anomaly_sustained_frames: default_anomaly_sustained(),
            dashcam_anomaly_cooldown_sec: default_anomaly_cooldown(),
            dashcam_anomaly_noise_floor: default_anomaly_noise(),
            dashcam_explicit: false,
        }
    }
}

impl SessionConfig {
    /// Merge a partial config update (from spatial_config tool) into this config.
    /// Only fields present in the update are overwritten.
    pub fn apply_update(&mut self, update: &ConfigUpdate) {
        if let Some(ref v) = update.static_patterns {
            self.static_patterns = v.clone();
        }
        if let Some(ref v) = update.state_properties {
            self.state_properties = v.clone();
        }
        if let Some(v) = update.cluster_by {
            self.cluster_by = v;
        }
        if let Some(v) = update.bearing_format {
            self.bearing_format = v;
        }
        if let Some(v) = update.expose_internals {
            self.expose_internals = v;
        }
        if let Some(v) = update.poll_interval {
            self.poll_interval = v;
        }
        if let Some(v) = update.token_hard_cap {
            self.token_hard_cap = v;
        }
        if let Some(v) = update.dashcam_enabled {
            self.dashcam_enabled = v;
        }
        if let Some(v) = update.dashcam_capture_interval {
            self.dashcam_capture_interval = v;
        }
        if let Some(v) = update.dashcam_pre_window_system_sec {
            self.dashcam_pre_window_system_sec = v;
        }
        if let Some(v) = update.dashcam_pre_window_deliberate_sec {
            self.dashcam_pre_window_deliberate_sec = v;
        }
        if let Some(v) = update.dashcam_post_window_system_sec {
            self.dashcam_post_window_system_sec = v;
        }
        if let Some(v) = update.dashcam_post_window_deliberate_sec {
            self.dashcam_post_window_deliberate_sec = v;
        }
        if let Some(v) = update.dashcam_max_window_sec {
            self.dashcam_max_window_sec = v;
        }
        if let Some(v) = update.dashcam_min_after_sec {
            self.dashcam_min_after_sec = v;
        }
        if let Some(v) = update.dashcam_system_min_interval_sec {
            self.dashcam_system_min_interval_sec = v;
        }
        if let Some(v) = update.dashcam_byte_cap_mb {
            self.dashcam_byte_cap_mb = v;
        }
        if let Some(v) = update.dashcam_screenshot_interval_frames {
            self.dashcam_screenshot_interval_frames = v;
        }
        if let Some(v) = update.dashcam_screenshot_quality {
            self.dashcam_screenshot_quality = v;
        }
        if let Some(v) = update.dashcam_screenshot_max_dimension {
            self.dashcam_screenshot_max_dimension = v;
        }
        if let Some(v) = update.dashcam_screenshot_byte_cap_mb {
            self.dashcam_screenshot_byte_cap_mb = v;
        }
        if let Some(v) = update.dashcam_dense_burst_enabled {
            self.dashcam_dense_burst_enabled = v;
        }
        if let Some(v) = update.dashcam_dense_burst_interval_frames {
            self.dashcam_dense_burst_interval_frames = v;
        }
        if let Some(v) = update.dashcam_dense_burst_duration_sec {
            self.dashcam_dense_burst_duration_sec = v;
        }
        if let Some(v) = update.dashcam_anomaly_enabled {
            self.dashcam_anomaly_enabled = v;
        }
        if let Some(v) = update.dashcam_anomaly_min_proportion {
            self.dashcam_anomaly_min_proportion = v;
        }
        if let Some(v) = update.dashcam_anomaly_relative_factor {
            self.dashcam_anomaly_relative_factor = v;
        }
        if let Some(v) = update.dashcam_anomaly_sustained_frames {
            self.dashcam_anomaly_sustained_frames = v;
        }
        if let Some(v) = update.dashcam_anomaly_cooldown_sec {
            self.dashcam_anomaly_cooldown_sec = v;
        }
        if let Some(v) = update.dashcam_anomaly_noise_floor {
            self.dashcam_anomaly_noise_floor = v;
        }
    }

    /// Check if a node path matches any static pattern (glob-style).
    /// Supports simple glob: "*" matches any segment, "walls/*" matches "walls/anything".
    pub fn matches_static_pattern(&self, path: &str) -> bool {
        self.static_patterns
            .iter()
            .any(|pattern| glob_match(pattern, path))
    }

    /// Filter state properties for a node based on its groups and class.
    /// Returns None if no filtering configured (include all exported vars).
    /// Returns Some(list) if specific properties should be included.
    pub fn filter_state_properties(&self, groups: &[String], class: &str) -> Option<Vec<String>> {
        if self.state_properties.is_empty() {
            return None; // No filtering — include all
        }

        let mut props = Vec::new();
        let mut found = false;

        // Check group-based entries
        for group in groups {
            if let Some(group_props) = self.state_properties.get(group) {
                props.extend(group_props.iter().cloned());
                found = true;
            }
        }

        // Check class-based entry
        if let Some(class_props) = self.state_properties.get(class) {
            props.extend(class_props.iter().cloned());
            found = true;
        }

        // Check wildcard
        if let Some(wildcard_props) = self.state_properties.get("*") {
            props.extend(wildcard_props.iter().cloned());
            found = true;
        }

        if found {
            props.sort();
            props.dedup();
            Some(props)
        } else {
            None // No matching rules — include all
        }
    }
}

/// Partial config update — all fields optional.
/// Used by the spatial_config MCP tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigUpdate {
    pub static_patterns: Option<Vec<String>>,
    pub state_properties: Option<HashMap<String, Vec<String>>>,
    pub cluster_by: Option<ClusterStrategy>,
    pub bearing_format: Option<BearingFormat>,
    pub expose_internals: Option<bool>,
    pub poll_interval: Option<u32>,
    pub token_hard_cap: Option<u32>,

    // Dashcam config overrides (M11)
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
    pub dashcam_screenshot_interval_frames: Option<u32>,
    pub dashcam_screenshot_quality: Option<f32>,
    pub dashcam_screenshot_max_dimension: Option<u32>,
    pub dashcam_screenshot_byte_cap_mb: Option<u32>,
    pub dashcam_dense_burst_enabled: Option<bool>,
    pub dashcam_dense_burst_interval_frames: Option<u32>,
    pub dashcam_dense_burst_duration_sec: Option<u32>,
    pub dashcam_anomaly_enabled: Option<bool>,
    pub dashcam_anomaly_min_proportion: Option<f64>,
    pub dashcam_anomaly_relative_factor: Option<f64>,
    pub dashcam_anomaly_sustained_frames: Option<u32>,
    pub dashcam_anomaly_cooldown_sec: Option<u32>,
    pub dashcam_anomaly_noise_floor: Option<u8>,
}

/// Simple glob matching for static patterns.
/// Supports: "walls/*" matches "walls/anything", "*" matches everything,
/// "walls/*/door" matches "walls/foo/door".
fn glob_match(pattern: &str, path: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();
    glob_match_parts(&pat_parts, &path_parts)
}

fn glob_match_parts(pattern: &[&str], path: &[&str]) -> bool {
    match (pattern.first(), path.first()) {
        (None, None) => true,
        (Some(&"*"), Some(_)) if pattern.len() == 1 => true, // trailing * matches any non-empty segment(s)
        (Some(&"*"), Some(_)) => {
            // * matches this segment, continue
            glob_match_parts(&pattern[1..], &path[1..])
        }
        (Some(p), Some(s)) if p == s => glob_match_parts(&pattern[1..], &path[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = SessionConfig::default();
        assert!(config.static_patterns.is_empty());
        assert!(config.state_properties.is_empty());
        assert_eq!(config.cluster_by, ClusterStrategy::Group);
        assert_eq!(config.bearing_format, BearingFormat::Both);
        assert!(!config.expose_internals);
        assert_eq!(config.poll_interval, 1);
        assert_eq!(config.token_hard_cap, 5000);
    }

    #[test]
    fn apply_partial_update() {
        let mut config = SessionConfig::default();
        let update = ConfigUpdate {
            static_patterns: Some(vec!["walls/*".into()]),
            ..Default::default()
        };
        config.apply_update(&update);
        assert_eq!(config.static_patterns, vec!["walls/*"]);
        // Other fields unchanged
        assert_eq!(config.token_hard_cap, 5000);
    }

    #[test]
    fn glob_match_simple() {
        assert!(glob_match("walls/*", "walls/segment_01"));
        assert!(glob_match("walls/*", "walls/door_02"));
        assert!(!glob_match("walls/*", "enemies/scout"));
        assert!(!glob_match("walls/*", "walls")); // no trailing segment
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("player", "player"));
        assert!(!glob_match("player", "players"));
    }

    #[test]
    fn glob_match_wildcard_all() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", "walls/segment_01"));
    }

    #[test]
    fn filter_state_properties_empty_config() {
        let config = SessionConfig::default();
        assert!(
            config
                .filter_state_properties(&["enemies".into()], "CharacterBody3D")
                .is_none()
        );
    }

    #[test]
    fn filter_state_properties_by_group() {
        let mut config = SessionConfig::default();
        config.state_properties.insert(
            "enemies".into(),
            vec!["health".into(), "alert_level".into()],
        );
        let result = config.filter_state_properties(&["enemies".into()], "CharacterBody3D");
        assert_eq!(result, Some(vec!["alert_level".into(), "health".into()]));
    }

    #[test]
    fn filter_state_properties_wildcard() {
        let mut config = SessionConfig::default();
        config
            .state_properties
            .insert("*".into(), vec!["visible".into()]);
        let result = config.filter_state_properties(&[], "Node3D");
        assert_eq!(result, Some(vec!["visible".into()]));
    }

    #[test]
    fn filter_state_properties_merged() {
        let mut config = SessionConfig::default();
        config
            .state_properties
            .insert("enemies".into(), vec!["health".into()]);
        config
            .state_properties
            .insert("*".into(), vec!["visible".into()]);
        let result = config.filter_state_properties(&["enemies".into()], "CharacterBody3D");
        let mut expected = vec!["health".into(), "visible".into()];
        expected.sort();
        assert_eq!(result, Some(expected));
    }
}
