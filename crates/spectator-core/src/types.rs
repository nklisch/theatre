use serde::{Deserialize, Serialize};

/// A 2D position in world space.
pub type Position2 = [f64; 2];

/// A 3D position in world space.
pub type Position3 = [f64; 3];

/// Rotation in degrees (yaw for 3D standard output).
pub type RotationDeg = f64;

/// Velocity vector.
pub type Velocity3 = [f64; 3];

/// 8-direction cardinal bearing relative to a perspective's facing direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinal {
    Ahead,
    AheadRight,
    Right,
    BehindRight,
    Behind,
    BehindLeft,
    Left,
    AheadLeft,
}

/// Elevation classification (3D only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Elevation {
    /// Target is within ±threshold meters (default 2m).
    Level,
    /// Target is above by N meters (rounded).
    #[serde(rename = "above")]
    Above(f64),
    /// Target is below by N meters (rounded).
    #[serde(rename = "below")]
    Below(f64),
}

/// Relative spatial position from a perspective to a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativePosition {
    /// Straight-line distance in world units.
    pub distance: f64,
    /// 8-direction cardinal bearing.
    pub bearing: Cardinal,
    /// Exact bearing in degrees (0 = ahead, clockwise).
    pub bearing_deg: f64,
    /// Elevation classification (3D only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<Elevation>,
    /// Whether line-of-sight is blocked (from camera perspective).
    pub occluded: bool,
}

/// Perspective from which spatial data is computed.
#[derive(Debug, Clone)]
pub struct Perspective {
    pub position: Position3,
    /// Forward direction vector (unit vector on XZ plane for 3D).
    pub forward: [f64; 3],
    /// Facing as cardinal label.
    pub facing: Cardinal,
    /// Facing as degrees from north (0=north/+Z, clockwise).
    pub facing_deg: f64,
}

/// Raw data for a single entity received from the addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEntityData {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub rotation_deg: [f64; 3],
    pub velocity: Velocity3,
    pub groups: Vec<String>,
    pub state: serde_json::Map<String, serde_json::Value>,
    pub visible: bool,
    pub is_static: bool,
    #[serde(default)]
    pub children: Vec<ChildInfo>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub signals_recent: Vec<RecentSignal>,
    #[serde(default)]
    pub signals_connected: Vec<String>,
    #[serde(default)]
    pub physics: Option<PhysicsData>,
    #[serde(default)]
    pub transform: Option<TransformData>,
}

/// Minimal child info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildInfo {
    pub name: String,
    pub class: String,
}

/// Recent signal emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentSignal {
    pub signal: String,
    pub frame: u64,
}

/// Physics state of a CharacterBody3D or RigidBody3D.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsData {
    pub velocity: Velocity3,
    pub on_floor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floor_normal: Option<[f64; 3]>,
    pub collision_layer: u32,
    pub collision_mask: u32,
}

/// Full transform data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformData {
    pub origin: Position3,
    pub basis: [[f64; 3]; 3],
    pub scale: [f64; 3],
}

/// Convert a `Vec<f64>` slice to a fixed `[f64; 2]`, filling missing elements with `0.0`.
pub fn vec_to_array2(v: &[f64]) -> [f64; 2] {
    [
        v.first().copied().unwrap_or(0.0),
        v.get(1).copied().unwrap_or(0.0),
    ]
}

/// Convert a `Vec<f64>` slice to a fixed `[f64; 3]`, filling missing elements with `0.0`.
///
/// Used to safely convert protocol `Vec<f64>` fields (position, rotation, velocity)
/// into the fixed-size arrays used by the core spatial logic.
pub fn vec_to_array3(v: &[f64]) -> [f64; 3] {
    [
        v.first().copied().unwrap_or(0.0),
        v.get(1).copied().unwrap_or(0.0),
        v.get(2).copied().unwrap_or(0.0),
    ]
}

/// Frame metadata from the addon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameInfo {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub delta: f64,
}
