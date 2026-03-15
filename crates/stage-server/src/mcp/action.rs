use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use stage_protocol::query::ActionRequest;

use super::require_param;

/// Spatial action type.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Pause or unpause the scene. Requires: paused (bool).
    Pause,
    /// Step N physics frames while paused. Requires: frames (int).
    AdvanceFrames,
    /// Step N seconds while paused. Requires: seconds (float).
    AdvanceTime,
    /// Move a node to a position. Requires: node, position. Optional: rotation_deg.
    Teleport,
    /// Change a node property. Requires: node, property, value.
    SetProperty,
    /// Call a method on a node. Requires: node, method. Optional: args.
    CallMethod,
    /// Emit a signal on a node. Requires: node, signal. Optional: args.
    EmitSignal,
    /// Instantiate a scene. Requires: scene_path, parent. Optional: name, position.
    SpawnNode,
    /// Delete a node. Requires: node.
    RemoveNode,
    /// Hold a named InputMap action. Requires: input_action. Optional: strength.
    ActionPress,
    /// Release a named InputMap action. Requires: input_action.
    ActionRelease,
    /// Inject a key press/release. Requires: keycode, pressed.
    InjectKey,
    /// Inject a mouse button press/release. Requires: button, pressed.
    InjectMouseButton,
}

/// MCP parameters for the spatial_action tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialActionParams {
    /// Action to perform.
    pub action: ActionType,

    /// Target node path (required for teleport, set_property, call_method,
    /// emit_signal, remove_node).
    pub node: Option<String>,

    /// For pause: whether to pause (true) or unpause (false).
    pub paused: Option<bool>,

    /// For advance_frames: number of physics frames to advance.
    pub frames: Option<u32>,

    /// For advance_time: seconds to advance.
    pub seconds: Option<f64>,

    /// For teleport: target position [x, y, z] or [x, y].
    pub position: Option<Vec<f64>>,

    /// For teleport: target rotation in degrees (yaw for 3D, angle for 2D).
    pub rotation_deg: Option<f64>,

    /// For set_property: property name.
    pub property: Option<String>,

    /// For set_property: new value.
    pub value: Option<serde_json::Value>,

    /// For emit_signal: signal name.
    pub signal: Option<String>,

    /// For emit_signal/call_method: arguments.
    pub args: Option<Vec<serde_json::Value>>,

    /// For call_method: method name.
    pub method: Option<String>,

    /// For spawn_node: scene resource path.
    pub scene_path: Option<String>,

    /// For spawn_node: parent node path.
    pub parent: Option<String>,

    /// For spawn_node: name for the new node.
    pub name: Option<String>,

    /// Whether to return a spatial_delta after the action (M4 placeholder).
    #[serde(default)]
    pub return_delta: bool,

    /// For action_press/action_release: InputMap action name (e.g. "jump").
    pub input_action: Option<String>,

    /// For action_press: strength 0.0–1.0 (default 1.0).
    pub strength: Option<f32>,

    /// For inject_key: Godot key name ("A", "SPACE", "UP", etc.).
    pub keycode: Option<String>,

    /// For inject_key/inject_mouse_button: whether pressed (true) or released (false).
    pub pressed: Option<bool>,

    /// For inject_key: whether this is an echo event.
    #[serde(default)]
    pub echo: bool,

    /// For inject_mouse_button: button name ("left", "right", "middle",
    /// "wheel_up", "wheel_down").
    pub button: Option<String>,
}

/// Build the addon ActionRequest from MCP params.
/// Validates required fields per action type.
pub fn build_action_request(params: &SpatialActionParams) -> Result<ActionRequest, McpError> {
    match &params.action {
        ActionType::Pause => {
            let paused = require_param!(
                params.paused,
                "'paused' (bool) is required for pause action"
            );
            Ok(ActionRequest::Pause { paused })
        }
        ActionType::AdvanceFrames => {
            let frames = require_param!(
                params.frames,
                "'frames' (int) is required for advance_frames action"
            );
            Ok(ActionRequest::AdvanceFrames { frames })
        }
        ActionType::AdvanceTime => {
            let seconds = require_param!(
                params.seconds,
                "'seconds' (float) is required for advance_time action"
            );
            Ok(ActionRequest::AdvanceTime { seconds })
        }
        ActionType::Teleport => {
            let node = require_param!(
                params.node.as_ref(),
                "'node' is required for teleport action"
            );
            let position = require_param!(
                params.position.as_ref(),
                "'position' ([x,y,z] or [x,y]) is required for teleport action"
            );
            Ok(ActionRequest::Teleport {
                path: node.clone(),
                position: position.clone(),
                rotation_deg: params.rotation_deg,
            })
        }
        ActionType::SetProperty => {
            let node = require_param!(
                params.node.as_ref(),
                "'node' is required for set_property action"
            );
            let property = require_param!(
                params.property.as_ref(),
                "'property' is required for set_property action"
            );
            let value = require_param!(
                params.value.as_ref(),
                "'value' is required for set_property action"
            );
            Ok(ActionRequest::SetProperty {
                path: node.clone(),
                property: property.clone(),
                value: value.clone(),
            })
        }
        ActionType::CallMethod => {
            let node = require_param!(
                params.node.as_ref(),
                "'node' is required for call_method action"
            );
            let method = require_param!(
                params.method.as_ref(),
                "'method' is required for call_method action"
            );
            let args = params.args.as_ref().cloned().unwrap_or_default();
            Ok(ActionRequest::CallMethod {
                path: node.clone(),
                method: method.clone(),
                args,
            })
        }
        ActionType::EmitSignal => {
            let node = require_param!(
                params.node.as_ref(),
                "'node' is required for emit_signal action"
            );
            let signal = require_param!(
                params.signal.as_ref(),
                "'signal' is required for emit_signal action"
            );
            let args = params.args.as_ref().cloned().unwrap_or_default();
            Ok(ActionRequest::EmitSignal {
                path: node.clone(),
                signal: signal.clone(),
                args,
            })
        }
        ActionType::SpawnNode => {
            let scene_path = require_param!(
                params.scene_path.as_ref(),
                "'scene_path' is required for spawn_node action"
            );
            let parent = require_param!(
                params.parent.as_ref(),
                "'parent' is required for spawn_node action"
            );
            Ok(ActionRequest::SpawnNode {
                scene_path: scene_path.clone(),
                parent: parent.clone(),
                name: params.name.clone(),
                position: params.position.clone(),
            })
        }
        ActionType::RemoveNode => {
            let node = require_param!(
                params.node.as_ref(),
                "'node' is required for remove_node action"
            );
            Ok(ActionRequest::RemoveNode { path: node.clone() })
        }
        ActionType::ActionPress => {
            let input_action = require_param!(
                params.input_action.as_ref(),
                "'input_action' is required for action_press"
            );
            Ok(ActionRequest::ActionPress {
                action_name: input_action.clone(),
                strength: params.strength.unwrap_or(1.0),
            })
        }
        ActionType::ActionRelease => {
            let input_action = require_param!(
                params.input_action.as_ref(),
                "'input_action' is required for action_release"
            );
            Ok(ActionRequest::ActionRelease {
                action_name: input_action.clone(),
            })
        }
        ActionType::InjectKey => {
            let keycode = require_param!(
                params.keycode.as_ref(),
                "'keycode' (e.g. \"SPACE\", \"W\") is required for inject_key"
            );
            let pressed = require_param!(
                params.pressed,
                "'pressed' (bool) is required for inject_key"
            );
            Ok(ActionRequest::InjectKey {
                keycode: keycode.clone(),
                pressed,
                echo: params.echo,
            })
        }
        ActionType::InjectMouseButton => {
            let button = require_param!(
                params.button.as_ref(),
                "'button' (\"left\", \"right\", \"middle\") is required for inject_mouse_button"
            );
            let pressed = require_param!(
                params.pressed,
                "'pressed' (bool) is required for inject_mouse_button"
            );
            Ok(ActionRequest::InjectMouseButton {
                button: button.clone(),
                pressed,
                position: params.position.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_params(action: ActionType) -> SpatialActionParams {
        SpatialActionParams {
            action,
            node: None,
            paused: None,
            frames: None,
            seconds: None,
            position: None,
            rotation_deg: None,
            property: None,
            value: None,
            signal: None,
            args: None,
            method: None,
            scene_path: None,
            parent: None,
            name: None,
            return_delta: false,
            input_action: None,
            strength: None,
            keycode: None,
            pressed: None,
            echo: false,
            button: None,
        }
    }

    #[test]
    fn build_action_request_pause() {
        let mut p = base_params(ActionType::Pause);
        p.paused = Some(true);
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::Pause { paused: true }));
    }

    #[test]
    fn build_action_request_teleport() {
        let mut p = base_params(ActionType::Teleport);
        p.node = Some("enemy".into());
        p.position = Some(vec![5.0, 0.0, -3.0]);
        p.rotation_deg = Some(90.0);
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::Teleport { .. }));
    }

    #[test]
    fn build_action_request_missing_node() {
        let mut p = base_params(ActionType::Teleport);
        p.position = Some(vec![5.0, 0.0, -3.0]);
        // node is None — should error
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn action_deserialize_invalid() {
        assert!(serde_json::from_str::<ActionType>(r#""fly""#).is_err());
    }

    #[test]
    fn build_action_request_call_method_with_args() {
        let mut p = base_params(ActionType::CallMethod);
        p.node = Some("player".into());
        p.method = Some("take_damage".into());
        p.args = Some(vec![serde_json::json!(50)]);
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::CallMethod { args, .. } if args.len() == 1));
    }

    #[test]
    fn build_action_request_action_press() {
        let mut p = base_params(ActionType::ActionPress);
        p.input_action = Some("jump".into());
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::ActionPress { action_name, strength } if action_name == "jump" && (strength - 1.0).abs() < 0.01)
        );
    }

    #[test]
    fn build_action_request_action_press_missing_input_action() {
        let p = base_params(ActionType::ActionPress);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_inject_key() {
        let mut p = base_params(ActionType::InjectKey);
        p.keycode = Some("SPACE".into());
        p.pressed = Some(true);
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::InjectKey { keycode, pressed: true, echo: false } if keycode == "SPACE")
        );
    }

    #[test]
    fn build_action_request_inject_key_missing_pressed() {
        let mut p = base_params(ActionType::InjectKey);
        p.keycode = Some("W".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_inject_mouse_button() {
        let mut p = base_params(ActionType::InjectMouseButton);
        p.button = Some("left".into());
        p.pressed = Some(true);
        p.position = Some(vec![100.0, 200.0]);
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::InjectMouseButton { button, pressed: true, .. } if button == "left")
        );
    }

    // --- Missing required param validation tests ---

    #[test]
    fn build_action_request_pause_missing_paused() {
        let p = base_params(ActionType::Pause);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_advance_frames_missing_frames() {
        let p = base_params(ActionType::AdvanceFrames);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_advance_time_missing_seconds() {
        let p = base_params(ActionType::AdvanceTime);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_teleport_missing_position() {
        let mut p = base_params(ActionType::Teleport);
        p.node = Some("player".into());
        // position is None — should error
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_set_property_missing_property() {
        let mut p = base_params(ActionType::SetProperty);
        p.node = Some("player".into());
        p.value = Some(serde_json::json!(42));
        // property is None — should error
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_set_property_missing_value() {
        let mut p = base_params(ActionType::SetProperty);
        p.node = Some("player".into());
        p.property = Some("health".into());
        // value is None — should error
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_set_property_missing_node() {
        let mut p = base_params(ActionType::SetProperty);
        p.property = Some("health".into());
        p.value = Some(serde_json::json!(42));
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_call_method_missing_method() {
        let mut p = base_params(ActionType::CallMethod);
        p.node = Some("player".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_call_method_missing_node() {
        let mut p = base_params(ActionType::CallMethod);
        p.method = Some("ping".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_call_method_no_args_defaults_empty() {
        let mut p = base_params(ActionType::CallMethod);
        p.node = Some("player".into());
        p.method = Some("ping".into());
        // args is None — should default to empty vec
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::CallMethod { args, .. } if args.is_empty()));
    }

    #[test]
    fn build_action_request_emit_signal_missing_node() {
        let mut p = base_params(ActionType::EmitSignal);
        p.signal = Some("health_changed".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_emit_signal_missing_signal() {
        let mut p = base_params(ActionType::EmitSignal);
        p.node = Some("player".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_emit_signal_no_args_defaults_empty() {
        let mut p = base_params(ActionType::EmitSignal);
        p.node = Some("player".into());
        p.signal = Some("ready".into());
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::EmitSignal { args, .. } if args.is_empty()));
    }

    #[test]
    fn build_action_request_emit_signal_with_args() {
        let mut p = base_params(ActionType::EmitSignal);
        p.node = Some("player".into());
        p.signal = Some("health_changed".into());
        p.args = Some(vec![serde_json::json!(50)]);
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::EmitSignal { args, .. } if args.len() == 1));
    }

    #[test]
    fn build_action_request_spawn_node_missing_scene_path() {
        let mut p = base_params(ActionType::SpawnNode);
        p.parent = Some("Enemies".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_spawn_node_missing_parent() {
        let mut p = base_params(ActionType::SpawnNode);
        p.scene_path = Some("res://enemy.tscn".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_spawn_node_with_optional_fields() {
        let mut p = base_params(ActionType::SpawnNode);
        p.scene_path = Some("res://enemy.tscn".into());
        p.parent = Some("Enemies".into());
        p.name = Some("TestEnemy".into());
        p.position = Some(vec![5.0, 0.0, -3.0]);
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::SpawnNode { name: Some(n), position: Some(pos), .. } if n == "TestEnemy" && pos.len() == 3)
        );
    }

    #[test]
    fn build_action_request_remove_node_missing_node() {
        let p = base_params(ActionType::RemoveNode);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_remove_node_valid() {
        let mut p = base_params(ActionType::RemoveNode);
        p.node = Some("Enemies/Scout".into());
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::RemoveNode { path } if path == "Enemies/Scout"));
    }

    #[test]
    fn build_action_request_action_release_missing_input_action() {
        let p = base_params(ActionType::ActionRelease);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_action_release_valid() {
        let mut p = base_params(ActionType::ActionRelease);
        p.input_action = Some("jump".into());
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::ActionRelease { action_name } if action_name == "jump")
        );
    }

    #[test]
    fn build_action_request_inject_key_missing_keycode() {
        let mut p = base_params(ActionType::InjectKey);
        p.pressed = Some(true);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_inject_mouse_button_missing_button() {
        let mut p = base_params(ActionType::InjectMouseButton);
        p.pressed = Some(true);
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_inject_mouse_button_missing_pressed() {
        let mut p = base_params(ActionType::InjectMouseButton);
        p.button = Some("left".into());
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_action_press_custom_strength() {
        let mut p = base_params(ActionType::ActionPress);
        p.input_action = Some("fire".into());
        p.strength = Some(0.5);
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::ActionPress { action_name, strength } if action_name == "fire" && (strength - 0.5).abs() < 0.01)
        );
    }

    #[test]
    fn build_action_request_inject_key_with_echo() {
        let mut p = base_params(ActionType::InjectKey);
        p.keycode = Some("A".into());
        p.pressed = Some(true);
        p.echo = true;
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::InjectKey { keycode, pressed: true, echo: true } if keycode == "A")
        );
    }

    #[test]
    fn build_action_request_inject_mouse_button_without_position() {
        let mut p = base_params(ActionType::InjectMouseButton);
        p.button = Some("right".into());
        p.pressed = Some(false);
        let req = build_action_request(&p).unwrap();
        assert!(
            matches!(req, ActionRequest::InjectMouseButton { button, pressed: false, position: None } if button == "right")
        );
    }
}
