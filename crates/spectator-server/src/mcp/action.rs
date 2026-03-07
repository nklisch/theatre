use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use spectator_protocol::query::ActionRequest;

use super::require_param;

/// MCP parameters for the spatial_action tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialActionParams {
    /// Action type: pause, advance_frames, advance_time, teleport, set_property,
    /// call_method, emit_signal, spawn_node, remove_node.
    #[schemars(
        description = "Action type: pause, advance_frames, advance_time, teleport, set_property, call_method, emit_signal, spawn_node, remove_node"
    )]
    pub action: String,

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

    /// For call_method: method arguments (alias for args).
    pub method_args: Option<Vec<serde_json::Value>>,

    /// For spawn_node: scene resource path.
    pub scene_path: Option<String>,

    /// For spawn_node: parent node path.
    pub parent: Option<String>,

    /// For spawn_node: name for the new node.
    pub name: Option<String>,

    /// Whether to return a spatial_delta after the action (M4 placeholder).
    #[serde(default)]
    pub return_delta: bool,
}

/// Build the addon ActionRequest from MCP params.
/// Validates required fields per action type.
pub fn build_action_request(params: &SpatialActionParams) -> Result<ActionRequest, McpError> {
    match params.action.as_str() {
        "pause" => {
            let paused = require_param!(params.paused, "'paused' (bool) is required for pause action");
            Ok(ActionRequest::Pause { paused })
        }
        "advance_frames" => {
            let frames = require_param!(params.frames, "'frames' (int) is required for advance_frames action");
            Ok(ActionRequest::AdvanceFrames { frames })
        }
        "advance_time" => {
            let seconds = require_param!(params.seconds, "'seconds' (float) is required for advance_time action");
            Ok(ActionRequest::AdvanceTime { seconds })
        }
        "teleport" => {
            let node = require_param!(params.node.as_ref(), "'node' is required for teleport action");
            let position = require_param!(params.position.as_ref(), "'position' ([x,y,z] or [x,y]) is required for teleport action");
            Ok(ActionRequest::Teleport {
                path: node.clone(),
                position: position.clone(),
                rotation_deg: params.rotation_deg,
            })
        }
        "set_property" => {
            let node = require_param!(params.node.as_ref(), "'node' is required for set_property action");
            let property = require_param!(params.property.as_ref(), "'property' is required for set_property action");
            let value = require_param!(params.value.as_ref(), "'value' is required for set_property action");
            Ok(ActionRequest::SetProperty {
                path: node.clone(),
                property: property.clone(),
                value: value.clone(),
            })
        }
        "call_method" => {
            let node = require_param!(params.node.as_ref(), "'node' is required for call_method action");
            let method = require_param!(params.method.as_ref(), "'method' is required for call_method action");
            let args = params
                .method_args
                .as_ref()
                .or(params.args.as_ref())
                .cloned()
                .unwrap_or_default();
            Ok(ActionRequest::CallMethod {
                path: node.clone(),
                method: method.clone(),
                args,
            })
        }
        "emit_signal" => {
            let node = require_param!(params.node.as_ref(), "'node' is required for emit_signal action");
            let signal = require_param!(params.signal.as_ref(), "'signal' is required for emit_signal action");
            let args = params.args.as_ref().cloned().unwrap_or_default();
            Ok(ActionRequest::EmitSignal {
                path: node.clone(),
                signal: signal.clone(),
                args,
            })
        }
        "spawn_node" => {
            let scene_path = require_param!(params.scene_path.as_ref(), "'scene_path' is required for spawn_node action");
            let parent = require_param!(params.parent.as_ref(), "'parent' is required for spawn_node action");
            Ok(ActionRequest::SpawnNode {
                scene_path: scene_path.clone(),
                parent: parent.clone(),
                name: params.name.clone(),
                position: params.position.clone(),
            })
        }
        "remove_node" => {
            let node = require_param!(params.node.as_ref(), "'node' is required for remove_node action");
            Ok(ActionRequest::RemoveNode { path: node.clone() })
        }
        other => Err(McpError::invalid_params(
            format!(
                "Unknown action type: '{other}'. Valid actions: \
                 pause, advance_frames, advance_time, teleport, set_property, \
                 call_method, emit_signal, spawn_node, remove_node"
            ),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_params(action: &str) -> SpatialActionParams {
        SpatialActionParams {
            action: action.into(),
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
            method_args: None,
            scene_path: None,
            parent: None,
            name: None,
            return_delta: false,
        }
    }

    #[test]
    fn build_action_request_pause() {
        let mut p = base_params("pause");
        p.paused = Some(true);
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::Pause { paused: true }));
    }

    #[test]
    fn build_action_request_teleport() {
        let mut p = base_params("teleport");
        p.node = Some("enemy".into());
        p.position = Some(vec![5.0, 0.0, -3.0]);
        p.rotation_deg = Some(90.0);
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::Teleport { .. }));
    }

    #[test]
    fn build_action_request_missing_node() {
        let mut p = base_params("teleport");
        p.position = Some(vec![5.0, 0.0, -3.0]);
        // node is None — should error
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_unknown_action() {
        let p = base_params("fly");
        assert!(build_action_request(&p).is_err());
    }

    #[test]
    fn build_action_request_call_method_uses_method_args() {
        let mut p = base_params("call_method");
        p.node = Some("player".into());
        p.method = Some("take_damage".into());
        p.method_args = Some(vec![serde_json::json!(50)]);
        let req = build_action_request(&p).unwrap();
        assert!(matches!(req, ActionRequest::CallMethod { args, .. } if args.len() == 1));
    }
}
