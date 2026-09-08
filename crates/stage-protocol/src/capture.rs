//! One launch policy shared by the launcher, engine and client schemas.
//! Resolving a policy is pure: it never starts capture or writes preferences.

use std::collections::BTreeMap;

use crate::dashcam::{DashcamConfig, ScreenshotReadback};
use serde::{Deserialize, Serialize};

/// Godot's JSON parser represents every number as a float. Normalize exact,
/// safely representable integers at that boundary, never round fractional data.
pub fn normalize_godot_integers(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => object.values_mut().for_each(normalize_godot_integers),
        serde_json::Value::Array(array) => array.iter_mut().for_each(normalize_godot_integers),
        serde_json::Value::Number(number) if number.is_f64() => {
            if let Some(n) = number.as_f64()
                && n.fract() == 0.0
                && n.abs() <= 9_007_199_254_740_991.0
            {
                *value = serde_json::json!(n as i64);
            }
        }
        _ => {}
    }
}

pub fn from_godot_json<T: serde::de::DeserializeOwned>(
    encoded: &str,
) -> Result<T, serde_json::Error> {
    let mut value = serde_json::from_str(encoded)?;
    normalize_godot_integers(&mut value);
    serde_json::from_value(value)
}

macro_rules! choices {
    ($name:ident { $($variant:ident),+ }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

choices!(Preset {
    Off,
    Minimal,
    Light,
    Standard,
    Heavy
});
choices!(Operator { Human, Agent });
choices!(Readiness { Scene, Project });
choices!(PlayStart { Manual, Ready });
choices!(Retention { Session, Rolling });
choices!(SavePolicy {
    Manual,
    OnStop,
    OnTrigger
});

/// Omitted fields preserve lower-precedence choices. An explicit off preset
/// disables the phase; it is not a quality setting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct PhaseOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<Preset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spatial: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<bool>,
    /// Node paths or `group:name` selectors. Empty means the whole scene only
    /// for Heavy. Light and Standard require a declared scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spatial_interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_size: Option<u32>,
    /// Blocking readback is an explicit recovery choice, never a preset default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_readback: Option<ScreenshotReadback>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movement_nodes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_actions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_min_proportion: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_relative_factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_sustained_frames: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_cooldown_sec: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomaly_noise_floor: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_records: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_mib: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<Retention>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<SavePolicy>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct LaunchOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observe: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<Operator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Readiness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play_start: Option<PlayStart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup: Option<PhaseOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub play: Option<PhaseOptions>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PhaseConfig {
    pub preset: Preset,
    pub enabled: bool,
    pub metrics: bool,
    pub spatial: bool,
    pub images: bool,
    pub scope: Vec<String>,
    /// Intervals are physics ticks, not a promise about observed wall-clock FPS.
    pub spatial_interval: u32,
    pub image_interval: u32,
    pub image_size: u32,
    pub image_readback: ScreenshotReadback,
    pub movement_nodes: Vec<String>,
    pub input_actions: Vec<String>,
    pub anomaly_enabled: bool,
    pub anomaly_min_proportion: f64,
    pub anomaly_relative_factor: f64,
    pub anomaly_sustained_frames: u32,
    pub anomaly_cooldown_sec: u32,
    pub anomaly_noise_floor: u8,
    pub duration_secs: u32,
    pub max_records: u32,
    /// Encoded payload budget, not total process memory.
    pub payload_mib: u32,
    pub retention: Retention,
    pub save: SavePolicy,
}

impl Default for PhaseConfig {
    fn default() -> Self {
        Self {
            preset: Preset::Off,
            enabled: false,
            metrics: false,
            spatial: false,
            images: false,
            scope: Vec::new(),
            spatial_interval: 6,
            image_interval: 12,
            image_size: 640,
            image_readback: ScreenshotReadback::Auto,
            movement_nodes: Vec::new(),
            input_actions: Vec::new(),
            anomaly_enabled: false,
            anomaly_min_proportion: 0.30,
            anomaly_relative_factor: 4.0,
            anomaly_sustained_frames: 4,
            anomaly_cooldown_sec: 30,
            anomaly_noise_floor: 24,
            duration_secs: 120,
            max_records: 12_000,
            payload_mib: 8,
            retention: Retention::Session,
            save: SavePolicy::Manual,
        }
    }
}

impl PhaseConfig {
    fn apply(
        &mut self,
        patch: &PhaseOptions,
        phase: &str,
        source: &str,
        sources: &mut BTreeMap<String, String>,
    ) {
        if let Some(preset) = patch.preset {
            self.preset = preset;
            self.enabled = preset != Preset::Off;
            self.metrics = self.enabled;
            self.spatial = matches!(preset, Preset::Light | Preset::Standard | Preset::Heavy);
            self.images = matches!(preset, Preset::Standard | Preset::Heavy);
            (
                self.spatial_interval,
                self.image_interval,
                self.image_size,
                self.payload_mib,
            ) = match preset {
                Preset::Off | Preset::Minimal => (6, 12, 640, 8),
                Preset::Light => (6, 12, 640, 128),
                Preset::Standard => (2, 12, 640, 128),
                Preset::Heavy => (1, 6, 960, 256),
            };
            for field in [
                "preset",
                "enabled",
                "metrics",
                "spatial",
                "images",
                "spatial_interval",
                "image_interval",
                "image_size",
                "payload_mib",
            ] {
                sources.insert(format!("{phase}.{field}"), source.into());
            }
        }
        macro_rules! apply {
            ($($field:ident),+) => { $(
                if let Some(value) = &patch.$field {
                    self.$field.clone_from(value);
                    sources.insert(format!("{phase}.{}", stringify!($field)), source.into());
                }
            )+ };
        }
        apply!(
            metrics,
            spatial,
            images,
            scope,
            spatial_interval,
            image_interval,
            image_size,
            image_readback,
            movement_nodes,
            input_actions,
            anomaly_enabled,
            anomaly_min_proportion,
            anomaly_relative_factor,
            anomaly_sustained_frames,
            anomaly_cooldown_sec,
            anomaly_noise_floor,
            duration_secs,
            max_records,
            payload_mib,
            retention,
            save
        );
    }

    pub fn validate(&self, phase: &str) -> Result<(), String> {
        if self.enabled != (self.preset != Preset::Off) {
            return Err(format!(
                "{phase}: enabled must agree with the selected preset"
            ));
        }
        DashcamConfig {
            movement_nodes: self.movement_nodes.clone(),
            anomaly_min_proportion: self.anomaly_min_proportion,
            anomaly_relative_factor: self.anomaly_relative_factor,
            anomaly_sustained_frames: self.anomaly_sustained_frames,
            anomaly_cooldown_sec: self.anomaly_cooldown_sec,
            anomaly_noise_floor: self.anomaly_noise_floor,
            input_actions: self.input_actions.clone(),
            ..Default::default()
        }
        .validate()?;
        if self.enabled
            && (!self.movement_nodes.is_empty() || !self.input_actions.is_empty())
            && !self.spatial
        {
            return Err(format!(
                "{phase}: movement evidence requires spatial capture"
            ));
        }
        if self.enabled && self.anomaly_enabled && !self.images {
            return Err(format!("{phase}: visual anomalies require images"));
        }
        if self.movement_nodes.iter().any(|s| {
            s.starts_with('/')
                || s.contains(':')
                || s.contains('\\')
                || s.split('/').any(|part| part == "..")
        }) {
            return Err(format!(
                "{phase}: movement_nodes must be scene-relative paths"
            ));
        }
        if [
            self.spatial_interval,
            self.image_interval,
            self.duration_secs,
            self.max_records,
            self.payload_mib,
        ]
        .contains(&0)
        {
            return Err(format!(
                "{phase}: intervals and capture bounds must be positive"
            ));
        }
        if self.duration_secs > 86_400 || self.max_records > 1_000_000 || self.payload_mib > 1024 {
            return Err(format!(
                "{phase}: maximum bounds are 86400 seconds, 1000000 records and 1024 MiB"
            ));
        }
        if !(1..=8192).contains(&self.image_size) {
            return Err(format!("{phase}: image_size must be between 1 and 8192"));
        }
        if !self.enabled && (self.metrics || self.spatial || self.images) {
            return Err(format!(
                "{phase}: select a capture preset before enabling channels"
            ));
        }
        if self.enabled && !(self.metrics || self.spatial || self.images) {
            return Err(format!(
                "{phase}: select at least one channel, or use preset off"
            ));
        }
        if self.enabled && self.spatial && self.scope.is_empty() && self.preset != Preset::Heavy {
            return Err(format!(
                "{phase}: spatial scope is required; select node paths, group:name, or Heavy for the whole scene"
            ));
        }
        if self.scope.len() > 64
            || self.scope.iter().any(|s| {
                s.is_empty()
                    || s.len() > 256
                    || s == "group:"
                    || s.split('/').any(|part| part == "..")
                    || s.starts_with('/')
                    || s.contains('\\')
                    || (s.contains(':') && !s.starts_with("group:"))
            })
        {
            return Err(format!(
                "{phase}: scope accepts up to 64 scene-relative node paths or group:name selectors"
            ));
        }
        if self.save == SavePolicy::OnTrigger && self.retention != Retention::Rolling {
            return Err(format!(
                "{phase}: on_trigger saving requires rolling retention"
            ));
        }
        if phase == "startup" && self.retention != Retention::Session {
            return Err("startup: loading capture uses bounded session retention".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LaunchConfig {
    pub observe: bool,
    pub operator: Operator,
    pub readiness: Readiness,
    pub play_start: PlayStart,
    pub startup: PhaseConfig,
    pub play: PhaseConfig,
    /// Source labels only, never machine-local configuration paths.
    pub sources: BTreeMap<String, String>,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            observe: false,
            operator: Operator::Human,
            readiness: Readiness::Project,
            play_start: PlayStart::Manual,
            startup: PhaseConfig {
                save: SavePolicy::OnStop,
                ..Default::default()
            },
            play: PhaseConfig::default(),
            sources: BTreeMap::new(),
        }
    }
}

impl LaunchConfig {
    /// Stage a partial next-play change against this run, not reread disk defaults.
    pub fn configure_play(&self, options: &PhaseOptions) -> Result<Self, String> {
        let mut next = self.clone();
        next.play
            .apply(options, "play", "runtime", &mut next.sources);
        next.play.validate("play")?;
        Ok(next)
    }
}

/// Sources must be supplied in increasing precedence. Validation happens only
/// after all layers, so higher-precedence corrections can repair lower layers.
pub fn resolve(layers: &[(&str, LaunchOptions)]) -> Result<LaunchConfig, String> {
    resolve_from(LaunchConfig::default(), layers)
}

/// Apply per-run arguments to an already resolved editor plan without rereading
/// preference files after launch has been accepted.
pub fn resolve_from(
    mut config: LaunchConfig,
    layers: &[(&str, LaunchOptions)],
) -> Result<LaunchConfig, String> {
    let mut start = config
        .sources
        .contains_key("play_start")
        .then_some(config.play_start);
    for (source, patch) in layers {
        macro_rules! apply {
            ($($field:ident),+) => { $(
                if let Some(value) = patch.$field {
                    config.$field = value;
                    config.sources.insert(stringify!($field).into(), (*source).into());
                }
            )+ };
        }
        apply!(observe, operator, readiness);
        if let Some(value) = patch.play_start {
            start = Some(value);
            config.sources.insert("play_start".into(), (*source).into());
        }
        if let Some(phase) = &patch.startup {
            config
                .startup
                .apply(phase, "startup", source, &mut config.sources);
        }
        if let Some(phase) = &patch.play {
            config
                .play
                .apply(phase, "play", source, &mut config.sources);
        }
    }
    config.play_start = start.unwrap_or(match config.operator {
        Operator::Human => PlayStart::Manual,
        Operator::Agent => PlayStart::Ready,
    });
    config.startup.validate("startup")?;
    config.play.validate("play")?;
    Ok(config)
}

/// Existing stage.toml connection/tracking sections stay owned by their readers.
pub fn parse_project_config(contents: &str) -> Result<LaunchOptions, String> {
    #[derive(Deserialize)]
    struct Document {
        #[serde(default)]
        launch: LaunchOptions,
    }
    toml::from_str::<Document>(contents)
        .map(|d| d.launch)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn options(value: serde_json::Value) -> LaunchOptions {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn ordinary_launch_and_operator_selection_do_not_capture() {
        for operator in ["human", "agent"] {
            let config = resolve(&[("launch", options(json!({"operator":operator})))]).unwrap();
            assert!(!config.observe && !config.startup.enabled && !config.play.enabled);
        }
    }

    #[test]
    fn phases_and_operator_defaults_are_independent() {
        let config = resolve(&[("launch", options(json!({"operator":"agent", "startup":{"preset":"minimal"}, "play":{"preset":"heavy", "images":false}})))]).unwrap();
        assert!(config.startup.metrics && !config.startup.spatial && !config.startup.images);
        assert!(config.play.spatial && !config.play.images);
        assert_eq!(config.play_start, PlayStart::Ready);
        assert_eq!(config.startup.save, SavePolicy::OnStop);
    }

    #[test]
    fn local_preferences_survive_operator_defaults_but_explicit_off_wins() {
        let local =
            options(json!({"observe":true, "play_start":"ready", "play":{"preset":"minimal"}}));
        let launch = options(json!({"operator":"human", "observe":false, "play":{"preset":"off"}}));
        let config = resolve(&[("local", local), ("launch", launch)]).unwrap();
        assert!(!config.observe && !config.play.enabled);
        assert_eq!(config.play_start, PlayStart::Ready);
        assert_eq!(config.sources["play_start"], "local");
        assert_eq!(config.sources["play.preset"], "launch");
    }

    #[test]
    fn invalid_combinations_fail_before_any_runtime_mutation() {
        for value in [
            json!({"play":{"preset":"light"}}),
            json!({"play":{"preset":"minimal", "duration_secs":0}}),
            json!({"play":{"images":true}}),
            json!({"play":{"preset":"minimal", "save":"on_trigger"}}),
            json!({"startup":{"preset":"minimal", "retention":"rolling"}}),
            json!({"play":{"preset":"light", "scope":["../outside"]}}),
        ] {
            assert!(
                resolve(&[("launch", options(value.clone()))]).is_err(),
                "{value}"
            );
        }
        assert!(serde_json::from_value::<LaunchOptions>(json!({"startpu":{}})).is_err());
    }

    #[test]
    fn project_toml_preserves_other_sections_and_rejects_misspelled_launch_fields() {
        let patch = parse_project_config("[tracking]\npoll_interval=5\n[launch]\nreadiness='project'\n[launch.play]\npreset='standard'\nscope=['group:review']\nimages=false\n").unwrap();
        let config = resolve(&[("project", patch)]).unwrap();
        assert_eq!(config.readiness, Readiness::Project);
        assert!(config.play.spatial && !config.play.images);
        assert!(parse_project_config("[launch]\nraediness='project'").is_err());
    }

    #[test]
    fn preset_names_roundtrip_and_manual_human_start_is_default() {
        let config = resolve(&[("launch", options(json!({"play":{"preset":"minimal"}})))]).unwrap();
        assert_eq!(config.play_start, PlayStart::Manual);
        assert_eq!(
            serde_json::to_value(config).unwrap()["play"]["preset"],
            "minimal"
        );
    }

    #[test]
    fn runtime_patch_preserves_run_overrides_and_rejects_invalid_candidates() {
        let original = resolve(&[(
            "launch",
            options(json!({"play":{"preset":"heavy","images":false,"duration_secs":23}})),
        )])
        .unwrap();
        let next = original
            .configure_play(&serde_json::from_value(json!({"spatial_interval":7})).unwrap())
            .unwrap();
        assert!(!next.play.images);
        assert_eq!(next.play.duration_secs, 23);
        assert_eq!(next.sources["play.spatial_interval"], "runtime");
        assert_eq!(next.sources["play.images"], "launch");
        assert!(
            next.configure_play(&serde_json::from_value(json!({"image_size":0})).unwrap())
                .is_err()
        );
        assert_eq!(original.play.spatial_interval, 1);
    }

    #[test]
    fn godot_numbers_are_normalized_without_rounding_invalid_options() {
        let options: PhaseOptions =
            from_godot_json(r#"{"duration_secs":12.0,"scope":[]}"#).unwrap();
        assert_eq!(options.duration_secs, Some(12));
        for invalid in [
            r#"{"duration_secs":12.5}"#,
            r#"{"duration_secs":-1.0}"#,
            r#"{"duration_secs":9007199254740992.0}"#,
        ] {
            assert!(from_godot_json::<PhaseOptions>(invalid).is_err());
        }
    }

    #[test]
    fn expensive_or_sensitive_channels_are_explicit_and_portable() {
        let config = resolve(&[("launch", options(json!({"play":{"preset":"heavy"}})))]).unwrap();
        assert_eq!(config.play.image_readback, ScreenshotReadback::Auto);
        assert!(config.play.input_actions.is_empty());
        assert!(!config.play.anomaly_enabled);
        for invalid in [
            json!({"play":{"preset":"minimal","movement_nodes":["Player"]}}),
            json!({"play":{"preset":"heavy","movement_nodes":["../Player"]}}),
            json!({"play":{"preset":"heavy","input_actions":["move"]}}),
            json!({"play":{"preset":"minimal","anomaly_enabled":true}}),
        ] {
            assert!(resolve(&[("launch", options(invalid))]).is_err());
        }
        let mut invalid = config.play;
        invalid.preset = Preset::Off;
        assert!(invalid.validate("play").is_err());
    }
}
