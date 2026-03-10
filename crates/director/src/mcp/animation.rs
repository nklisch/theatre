use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `animation_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the animation resource relative to project
    /// (e.g. "animations/walk.tres").
    pub resource_path: String,

    /// Animation length in seconds.
    pub length: f64,

    /// Loop mode: "none", "linear", or "pingpong". Default: "none".
    #[serde(default)]
    pub loop_mode: Option<String>,

    /// Snap step for keyframe times (seconds). Default: Godot's default (0.0333...).
    #[serde(default)]
    pub step: Option<f64>,
}

/// A single keyframe for `animation_add_track`.
///
/// The required fields depend on track_type:
/// - value: `value` (any JSON value — type-converted per track path), optional `transition`
/// - position_3d, scale_3d: `value` as {x, y, z}
/// - rotation_3d: `value` as {x, y, z, w} (quaternion)
/// - blend_shape: `value` as float
/// - method: `method` (string) and `args` (array)
/// - bezier: `value` (float), optional `in_handle` and `out_handle` as {x, y}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Keyframe {
    /// Time position in seconds.
    pub time: f64,

    /// Keyframe value. Required for value, position_3d, rotation_3d,
    /// scale_3d, blend_shape, and bezier tracks. For method tracks, omit
    /// and use `method`/`args` instead.
    #[serde(default)]
    pub value: Option<serde_json::Value>,

    /// Transition curve for value tracks. 1.0 = linear (default).
    /// <1 = ease in, >1 = ease out. Negative = back/elastic.
    #[serde(default)]
    pub transition: Option<f64>,

    /// Method name (method tracks only).
    #[serde(default)]
    pub method: Option<String>,

    /// Method arguments (method tracks only).
    #[serde(default)]
    pub args: Option<Vec<serde_json::Value>>,

    /// Bezier in-handle as {x, y} (bezier tracks only).
    /// x is time offset (negative), y is value offset.
    #[serde(default)]
    pub in_handle: Option<serde_json::Value>,

    /// Bezier out-handle as {x, y} (bezier tracks only).
    /// x is time offset (positive), y is value offset.
    #[serde(default)]
    pub out_handle: Option<serde_json::Value>,
}

/// Parameters for `animation_add_track`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationAddTrackParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the animation resource (relative to project).
    pub resource_path: String,

    /// Track type: "value", "position_3d", "rotation_3d", "scale_3d",
    /// "blend_shape", "method", or "bezier".
    pub track_type: String,

    /// Node path for the track, relative to the AnimationPlayer.
    /// For value tracks, include the property as a subpath
    /// (e.g. "Sprite2D:modulate", "../Player:position").
    /// For position/rotation/scale tracks, just the node path
    /// (e.g. "Mesh", "../Player").
    pub node_path: String,

    /// Keyframes to insert on the track.
    pub keyframes: Vec<Keyframe>,

    /// Interpolation type: "nearest", "linear", or "cubic".
    /// Default: "linear".
    #[serde(default)]
    pub interpolation: Option<String>,

    /// Update mode for value tracks: "continuous", "discrete",
    /// or "capture". Default: "continuous". Ignored for non-value tracks.
    #[serde(default)]
    pub update_mode: Option<String>,
}

/// Parameters for `animation_read`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationReadParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the animation resource (relative to project).
    pub resource_path: String,
}

/// Parameters for `animation_remove_track`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnimationRemoveTrackParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the animation resource (relative to project).
    pub resource_path: String,

    /// Track index to remove. Mutually exclusive with `node_path`.
    #[serde(default)]
    pub track_index: Option<u32>,

    /// Remove all tracks matching this node path. Mutually exclusive
    /// with `track_index`.
    #[serde(default)]
    pub node_path: Option<String>,
}
