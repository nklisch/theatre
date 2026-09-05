//! Recorder-owned settings and partial updates shared by MCP, TOML and Godot.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CapturePreset {
    /// Lower sampling frequency and smaller images for movement investigations.
    Lightweight,
    /// More frequent samples and larger images, with higher capture cost.
    Detailed,
}

// One field catalog keeps effective values, patch keys and defaults together.
macro_rules! settings {
    ($( $(#[$meta:meta])* $field:ident: $ty:ty = $default:expr; )*) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(default, deny_unknown_fields)]
        pub struct DashcamConfig { $( $(#[$meta])* pub $field: $ty, )* }

        impl Default for DashcamConfig {
            fn default() -> Self { Self { $( $field: $default, )* } }
        }

        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct DashcamConfigPatch {
            /// Apply a named capture setup, then any explicit overrides. Does not enable recording.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub preset: Option<CapturePreset>,
            $( $(#[$meta])* #[serde(skip_serializing_if = "Option::is_none")] pub $field: Option<$ty>, )*
        }

        impl DashcamConfigPatch {
            /// Validate the complete candidate before the recorder changes any state.
            pub fn apply_to(&self, current: &DashcamConfig) -> Result<DashcamConfig, String> {
                let mut next = current.clone();
                if let Some(preset) = self.preset { next.apply_preset(preset); }
                $( if let Some(value) = &self.$field { next.$field.clone_from(value); } )*
                next.validate()?;
                Ok(next)
            }
        }
    }
}

settings! {
    /// Explicitly start or stop recording. Presets do not change this field.
    enabled: bool = true;
    /// Spatial sampling interval in physics frames. Must be positive.
    capture_interval: u32 = 1;
    /// Up to 16 CharacterBody3D paths to sample for contact evidence. Empty disables it.
    movement_nodes: Vec<String> = Vec::new();
    /// Up to 16 named InputMap actions sampled on selected movement nodes, not raw input events.
    input_actions: Vec<String> = Vec::new();
    pre_window_system_sec: u32 = 30;
    pre_window_deliberate_sec: u32 = 60;
    post_window_system_sec: u32 = 10;
    post_window_deliberate_sec: u32 = 30;
    max_window_sec: u32 = 120;
    min_after_sec: u32 = 5;
    system_min_interval_sec: u32 = 2;
    byte_cap_mb: u32 = 1024;
    screenshot_enabled: bool = true;
    /// Screenshot sampling interval in physics frames. Must be positive.
    screenshot_interval_frames: u32 = 4;
    /// JPEG quality from zero to one.
    screenshot_quality: f32 = 0.65;
    /// Maximum output image dimension, from 1 to 8192 pixels.
    screenshot_max_dimension: u32 = 480;
    screenshot_byte_cap_mb: u32 = 32;
    screenshot_encode_queue: usize = 8;
    dense_burst_enabled: bool = false;
    dense_burst_interval_frames: u32 = 2;
    dense_burst_duration_sec: u32 = 15;
    anomaly_enabled: bool = true;
    anomaly_min_proportion: f64 = 0.30;
    anomaly_relative_factor: f64 = 4.0;
    anomaly_sustained_frames: u32 = 4;
    anomaly_cooldown_sec: u32 = 30;
    anomaly_noise_floor: u8 = 24;
}

impl DashcamConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("capture_interval", self.capture_interval),
            ("max_window_sec", self.max_window_sec),
            ("byte_cap_mb", self.byte_cap_mb),
            (
                "screenshot_interval_frames",
                self.screenshot_interval_frames,
            ),
            ("screenshot_byte_cap_mb", self.screenshot_byte_cap_mb),
            (
                "dense_burst_interval_frames",
                self.dense_burst_interval_frames,
            ),
            ("anomaly_sustained_frames", self.anomaly_sustained_frames),
        ] {
            if value == 0 {
                return Err(format!("{name} must be positive"));
            }
        }
        for (name, values) in [
            ("movement_nodes", &self.movement_nodes),
            ("input_actions", &self.input_actions),
        ] {
            if values.len() > 16
                || values
                    .iter()
                    .any(|value| value.is_empty() || value.len() > 256)
            {
                return Err(format!(
                    "{name} accepts at most 16 nonempty names of at most 256 bytes"
                ));
            }
            let unique: std::collections::HashSet<_> = values.iter().collect();
            if unique.len() != values.len() {
                return Err(format!("{name} must not contain duplicates"));
            }
        }
        if !self.input_actions.is_empty() && self.movement_nodes.is_empty() {
            return Err("input_actions requires at least one movement_nodes target".into());
        }
        if self.screenshot_encode_queue == 0 {
            return Err("screenshot_encode_queue must be positive".into());
        }
        if !(1..=8192).contains(&self.screenshot_max_dimension) {
            return Err("screenshot_max_dimension must be between 1 and 8192".into());
        }
        if !(0.0..=1.0).contains(&self.screenshot_quality) {
            return Err("screenshot_quality must be between 0 and 1".into());
        }
        if !(0.0..=1.0).contains(&self.anomaly_min_proportion) {
            return Err("anomaly_min_proportion must be between 0 and 1".into());
        }
        if !self.anomaly_relative_factor.is_finite() || self.anomaly_relative_factor < 1.0 {
            return Err("anomaly_relative_factor must be finite and at least 1".into());
        }
        Ok(())
    }

    /// Identify the preset-controlled fields without treating unrelated options as defaults.
    pub fn matching_preset(&self) -> Option<CapturePreset> {
        [CapturePreset::Lightweight, CapturePreset::Detailed]
            .into_iter()
            .find(|preset| {
                let mut candidate = self.clone();
                candidate.apply_preset(*preset);
                candidate == *self
            })
    }

    fn apply_preset(&mut self, preset: CapturePreset) {
        let (interval, screenshots, dimension, state_mb, image_mb) = match preset {
            CapturePreset::Lightweight => (6, 12, 640, 128, 16),
            CapturePreset::Detailed => (2, 6, 960, 256, 32),
        };
        self.capture_interval = interval;
        self.screenshot_interval_frames = screenshots;
        self.screenshot_max_dimension = dimension;
        self.byte_cap_mb = state_mb;
        self.screenshot_byte_cap_mb = image_mb;
        self.screenshot_enabled = true;
        self.dense_burst_enabled = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn partial_configuration_is_validated_without_changing_the_original() {
        let original = DashcamConfig::default();
        for value in [
            json!({"capture_interval":6, "screenshot_quality":2}),
            json!({"capture_interval":0}),
            json!({"screenshot_max_dimension":8193}),
        ] {
            let patch: DashcamConfigPatch = serde_json::from_value(value).unwrap();
            assert!(patch.apply_to(&original).is_err());
        }
        for value in [
            json!({"pre_window_sec":{"deliberate":15}}),
            json!({"enabled":"yes"}),
            json!({"capture_interval":u64::MAX}),
            json!({"anomaly_noise_floor":256}),
        ] {
            assert!(serde_json::from_value::<DashcamConfigPatch>(value).is_err());
        }
        let patch: DashcamConfigPatch =
            serde_json::from_value(json!({"dense_burst_duration_sec":7})).unwrap();
        let effective = patch.apply_to(&original).unwrap();
        assert_eq!(effective.dense_burst_duration_sec, 7);
        assert_eq!(effective.capture_interval, original.capture_interval);
        assert_eq!(original, DashcamConfig::default());
    }

    #[test]
    fn movement_selection_is_opt_in_and_bounded() {
        let current = DashcamConfig::default();
        assert!(current.movement_nodes.is_empty() && current.input_actions.is_empty());
        for value in [
            json!({"input_actions":["move"]}),
            json!({"movement_nodes":[""]}),
            json!({"movement_nodes":["Player", "Player"]}),
            json!({"movement_nodes":["Player"], "input_actions":vec!["move"; 17]}),
            json!({"movement_nodes":["Player"], "input_actions":["move", "move"]}),
        ] {
            let patch: DashcamConfigPatch = serde_json::from_value(value).unwrap();
            assert!(patch.apply_to(&current).is_err());
        }
        let patch: DashcamConfigPatch = serde_json::from_value(json!({
            "movement_nodes":["Player"], "input_actions":["move"]
        }))
        .unwrap();
        let enabled = patch.apply_to(&current).unwrap();
        assert_eq!(enabled.input_actions, ["move"]);
        let disabled: DashcamConfigPatch = serde_json::from_value(json!({
            "movement_nodes":[], "input_actions":[]
        }))
        .unwrap();
        assert_eq!(disabled.apply_to(&enabled).unwrap(), current);
    }

    #[test]
    fn presets_preserve_recording_state_and_explicit_overrides_win() {
        let original = DashcamConfig {
            enabled: false,
            ..Default::default()
        };
        let patch: DashcamConfigPatch = serde_json::from_value(json!({
            "preset":"lightweight", "screenshot_max_dimension":320
        }))
        .unwrap();
        let effective = patch.apply_to(&original).unwrap();
        assert!(!effective.enabled);
        assert_eq!(effective.capture_interval, 6);
        assert_eq!(effective.screenshot_interval_frames, 12);
        assert_eq!(effective.screenshot_max_dimension, 320);
    }
}
