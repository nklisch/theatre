use serde::{Deserialize, Serialize};

/// Scene coordinate system type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneDimensions {
    /// Pure 2D scene (Node2D root, Camera2D).
    Two,
    /// Pure 3D scene (Node3D root, Camera3D).
    Three,
    /// Mixed scene containing both Node2D and Node3D subtrees.
    Mixed,
}

impl SceneDimensions {
    pub fn is_2d(&self) -> bool {
        matches!(self, Self::Two)
    }

    pub fn is_3d(&self) -> bool {
        matches!(self, Self::Three)
    }

    pub fn is_mixed(&self) -> bool {
        matches!(self, Self::Mixed)
    }

    pub fn from_u32(v: u32) -> Self {
        match v {
            2 => Self::Two,
            3 => Self::Three,
            _ => Self::Mixed,
        }
    }
}

/// Sent by the addon immediately after TCP connection is established.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Handshake {
    /// Spectator addon version (e.g., "0.1.0")
    pub spectator_version: String,

    /// Wire protocol version. Must match between server and addon.
    pub protocol_version: u32,

    /// Godot engine version string (e.g., "4.3")
    pub godot_version: String,

    /// 2, 3, or 0 for mixed. Determined by scene root type.
    pub scene_dimensions: u32,

    /// Physics ticks per second (typically 60)
    pub physics_ticks_per_sec: u32,

    /// Godot project name from ProjectSettings
    pub project_name: String,
}

/// Sent by the server in response to a valid Handshake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeAck {
    /// Server's spectator version
    pub spectator_version: String,

    /// Agreed protocol version
    pub protocol_version: u32,

    /// Unique identifier for this session
    pub session_id: String,
}

/// Sent by the server when protocol versions are incompatible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeError {
    /// Human-readable error description
    pub message: String,

    /// Server's spectator version
    pub server_version: String,

    /// Protocol versions the server supports
    pub supported_protocols: Vec<u32>,
}

/// Current protocol version. Incremented on breaking wire format changes.
pub const PROTOCOL_VERSION: u32 = 1;

impl Handshake {
    pub fn new(
        godot_version: String,
        scene_dimensions: u32,
        physics_ticks_per_sec: u32,
        project_name: String,
    ) -> Self {
        Self {
            spectator_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
            godot_version,
            scene_dimensions,
            physics_ticks_per_sec,
            project_name,
        }
    }

    /// Return the scene dimensions as a typed enum.
    pub fn dimensions(&self) -> SceneDimensions {
        SceneDimensions::from_u32(self.scene_dimensions)
    }
}

impl HandshakeAck {
    pub fn new(session_id: String) -> Self {
        Self {
            spectator_version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: PROTOCOL_VERSION,
            session_id,
        }
    }
}

impl HandshakeError {
    pub fn version_mismatch(addon_version: u32) -> Self {
        Self {
            message: format!(
                "Protocol version mismatch: server supports v{}, addon sent v{}",
                PROTOCOL_VERSION, addon_version
            ),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            supported_protocols: vec![PROTOCOL_VERSION],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::Message;

    #[test]
    fn scene_dimensions_from_u32() {
        assert_eq!(SceneDimensions::from_u32(2), SceneDimensions::Two);
        assert_eq!(SceneDimensions::from_u32(3), SceneDimensions::Three);
        assert_eq!(SceneDimensions::from_u32(0), SceneDimensions::Mixed);
        assert_eq!(SceneDimensions::from_u32(99), SceneDimensions::Mixed);
    }

    #[test]
    fn scene_dimensions_predicates() {
        assert!(SceneDimensions::Two.is_2d());
        assert!(!SceneDimensions::Two.is_3d());
        assert!(SceneDimensions::Three.is_3d());
        assert!(SceneDimensions::Mixed.is_mixed());
    }

    #[test]
    fn handshake_dimensions_helper() {
        let h = Handshake::new("4.3".into(), 2, 60, "TestProject".into());
        assert_eq!(h.dimensions(), SceneDimensions::Two);
        let h3 = Handshake::new("4.3".into(), 3, 60, "TestProject".into());
        assert_eq!(h3.dimensions(), SceneDimensions::Three);
    }

    #[test]
    fn handshake_round_trip() {
        let h = Handshake::new("4.3".into(), 3, 60, "TestProject".into());
        let msg = Message::Handshake(h.clone());
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Message::Handshake(ref inner) if inner == &h));
    }

    #[test]
    fn handshake_has_type_tag() {
        let h = Handshake::new("4.3".into(), 3, 60, "TestProject".into());
        let msg = Message::Handshake(h);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"handshake""#));
    }

    #[test]
    fn handshake_ack_round_trip() {
        let ack = HandshakeAck::new("sess_abc123".into());
        let msg = Message::HandshakeAck(ack.clone());
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Message::HandshakeAck(ref inner) if inner == &ack));
    }

    #[test]
    fn handshake_error_round_trip() {
        let err = HandshakeError::version_mismatch(99);
        let msg = Message::HandshakeError(err.clone());
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Message::HandshakeError(ref inner) if inner == &err));
    }
}
