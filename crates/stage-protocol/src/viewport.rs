use serde::{Deserialize, Serialize};

use crate::runtime::RuntimeIdentity;

pub const DEFAULT_MAX_DIMENSION: u32 = 1280;
pub const MAX_DIMENSION: u32 = 2048;

fn default_max_dimension() -> u32 {
    DEFAULT_MAX_DIMENSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ViewportParams {
    /// Maximum output width or height, preserving aspect ratio without upscaling (1–2048).
    #[serde(default = "default_max_dimension")]
    #[cfg_attr(feature = "schema", schemars(range(min = 1, max = 2048)))]
    pub max_dimension: u32,
}

impl ViewportParams {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_DIMENSION).contains(&self.max_dimension) {
            return Err(format!(
                "max_dimension must be between 1 and {MAX_DIMENSION}"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ViewportAvailability {
    Available {
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
    },
    Unavailable {
        reason: ViewportUnavailableReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ViewportUnavailableReason {
    Headless,
    NoViewport,
    EmptyPixels,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ViewportMetadata {
    pub identity: RuntimeIdentity,
    /// Physics counter at readback, NOT the simulation frame represented by the pixels.
    pub readback_physics_frame: u64,
    /// Engine render counter at readback. Pixels are the latest completed render;
    /// this does not establish an atomic snapshot with a separate spatial query.
    pub frames_drawn: u64,
    pub timestamp_ms: u64,
    #[serde(flatten)]
    pub availability: ViewportAvailability,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ViewportCapture {
    pub metadata: ViewportMetadata,
    /// Native JPEG, present only for available pixels.
    pub image_base64: Option<String>,
}
