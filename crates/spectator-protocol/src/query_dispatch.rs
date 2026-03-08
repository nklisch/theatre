use crate::query::{
    ActionRequest, GetFrameInfoParams, GetNodeInspectParams, GetSceneTreeParams,
    GetSnapshotDataParams, SpatialQueryRequest,
};

/// Known query method names dispatched by the addon's TCP query handler.
///
/// Used to validate method names and deserialize params before reaching Godot.
/// This is the authoritative list — adding a new method requires updating this enum.
///
/// Methods prefixed `Recording*` are routed through `recording_handler.rs` rather
/// than `query_handler.rs`, but they are still valid wire methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMethod {
    // Spatial query methods (routed through query_handler.rs)
    GetSnapshotData,
    GetFrameInfo,
    GetNodeInspect,
    GetSceneTree,
    ExecuteAction,
    SpatialQuery,
    // Recording methods (routed through recording_handler.rs)
    RecordingStart,
    RecordingStop,
    RecordingStatus,
    RecordingMarker,
    RecordingMarkers,
    RecordingList,
    RecordingDelete,
    RecordingResolvePath,
}

impl QueryMethod {
    /// Resolve a wire method name to its enum variant. Returns `None` for unknown methods.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "get_snapshot_data" => Some(Self::GetSnapshotData),
            "get_frame_info" => Some(Self::GetFrameInfo),
            "get_node_inspect" => Some(Self::GetNodeInspect),
            "get_scene_tree" => Some(Self::GetSceneTree),
            "execute_action" => Some(Self::ExecuteAction),
            "spatial_query" => Some(Self::SpatialQuery),
            "recording_start" => Some(Self::RecordingStart),
            "recording_stop" => Some(Self::RecordingStop),
            "recording_status" => Some(Self::RecordingStatus),
            "recording_marker" => Some(Self::RecordingMarker),
            "recording_markers" => Some(Self::RecordingMarkers),
            "recording_list" => Some(Self::RecordingList),
            "recording_delete" => Some(Self::RecordingDelete),
            "recording_resolve_path" => Some(Self::RecordingResolvePath),
            _ => None,
        }
    }

    /// Wire name for this method (inverse of `from_str`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GetSnapshotData => "get_snapshot_data",
            Self::GetFrameInfo => "get_frame_info",
            Self::GetNodeInspect => "get_node_inspect",
            Self::GetSceneTree => "get_scene_tree",
            Self::ExecuteAction => "execute_action",
            Self::SpatialQuery => "spatial_query",
            Self::RecordingStart => "recording_start",
            Self::RecordingStop => "recording_stop",
            Self::RecordingStatus => "recording_status",
            Self::RecordingMarker => "recording_marker",
            Self::RecordingMarkers => "recording_markers",
            Self::RecordingList => "recording_list",
            Self::RecordingDelete => "recording_delete",
            Self::RecordingResolvePath => "recording_resolve_path",
        }
    }

    /// Returns true if this method is routed through `recording_handler.rs`.
    pub fn is_recording_method(self) -> bool {
        matches!(
            self,
            Self::RecordingStart
                | Self::RecordingStop
                | Self::RecordingStatus
                | Self::RecordingMarker
                | Self::RecordingMarkers
                | Self::RecordingList
                | Self::RecordingDelete
                | Self::RecordingResolvePath
        )
    }

    /// Validate that `params` deserialize correctly for this method.
    /// Returns `Ok(())` or a human-readable error string.
    ///
    /// Recording methods use ad-hoc JSON parsing in `recording_handler.rs` rather
    /// than typed structs, so only minimal structural validation is done for them.
    pub fn validate_params(self, params: &serde_json::Value) -> Result<(), String> {
        match self {
            Self::GetSnapshotData => serde_json::from_value::<GetSnapshotDataParams>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::GetFrameInfo => serde_json::from_value::<GetFrameInfoParams>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::GetNodeInspect => {
                serde_json::from_value::<GetNodeInspectParams>(params.clone())
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            Self::GetSceneTree => serde_json::from_value::<GetSceneTreeParams>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::ExecuteAction => serde_json::from_value::<ActionRequest>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::SpatialQuery => serde_json::from_value::<SpatialQueryRequest>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            // Recording methods: must be a JSON object (handler does field-level parsing)
            Self::RecordingStart => {
                if !params.is_object() {
                    return Err("recording_start params must be an object".into());
                }
                Ok(())
            }
            Self::RecordingStop
            | Self::RecordingStatus
            | Self::RecordingResolvePath => Ok(()), // no required params
            Self::RecordingMarker => {
                if !params.is_object() {
                    return Err("recording_marker params must be an object".into());
                }
                Ok(())
            }
            Self::RecordingMarkers | Self::RecordingList | Self::RecordingDelete => {
                if !params.is_null() && !params.is_object() {
                    return Err(format!("{} params must be an object or null", self.as_str()));
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- from_str ---

    #[test]
    fn all_known_methods_resolve() {
        let cases = [
            ("get_snapshot_data", QueryMethod::GetSnapshotData),
            ("get_frame_info", QueryMethod::GetFrameInfo),
            ("get_node_inspect", QueryMethod::GetNodeInspect),
            ("get_scene_tree", QueryMethod::GetSceneTree),
            ("execute_action", QueryMethod::ExecuteAction),
            ("spatial_query", QueryMethod::SpatialQuery),
            ("recording_start", QueryMethod::RecordingStart),
            ("recording_stop", QueryMethod::RecordingStop),
            ("recording_status", QueryMethod::RecordingStatus),
            ("recording_marker", QueryMethod::RecordingMarker),
            ("recording_markers", QueryMethod::RecordingMarkers),
            ("recording_list", QueryMethod::RecordingList),
            ("recording_delete", QueryMethod::RecordingDelete),
            ("recording_resolve_path", QueryMethod::RecordingResolvePath),
        ];
        for (name, expected) in cases {
            assert_eq!(QueryMethod::from_str(name), Some(expected), "failed for {name}");
        }
    }

    #[test]
    fn unknown_method_returns_none() {
        assert_eq!(QueryMethod::from_str("bogus"), None);
        assert_eq!(QueryMethod::from_str("get_snapshot"), None);
        assert_eq!(QueryMethod::from_str(""), None);
    }

    #[test]
    fn as_str_round_trips() {
        let methods = [
            QueryMethod::GetSnapshotData,
            QueryMethod::GetFrameInfo,
            QueryMethod::GetNodeInspect,
            QueryMethod::GetSceneTree,
            QueryMethod::ExecuteAction,
            QueryMethod::SpatialQuery,
            QueryMethod::RecordingStart,
            QueryMethod::RecordingStop,
            QueryMethod::RecordingStatus,
            QueryMethod::RecordingMarker,
            QueryMethod::RecordingMarkers,
            QueryMethod::RecordingList,
            QueryMethod::RecordingDelete,
            QueryMethod::RecordingResolvePath,
        ];
        for method in methods {
            let name = method.as_str();
            assert_eq!(QueryMethod::from_str(name), Some(method), "round-trip failed for {name}");
        }
    }

    // --- recording method dispatch ---

    #[test]
    fn recording_methods_are_flagged_correctly() {
        assert!(QueryMethod::RecordingStart.is_recording_method());
        assert!(QueryMethod::RecordingStop.is_recording_method());
        assert!(QueryMethod::RecordingStatus.is_recording_method());
        assert!(QueryMethod::RecordingMarker.is_recording_method());
        assert!(QueryMethod::RecordingMarkers.is_recording_method());
        assert!(QueryMethod::RecordingList.is_recording_method());
        assert!(QueryMethod::RecordingDelete.is_recording_method());
        assert!(QueryMethod::RecordingResolvePath.is_recording_method());
    }

    #[test]
    fn spatial_methods_are_not_recording_methods() {
        assert!(!QueryMethod::GetSnapshotData.is_recording_method());
        assert!(!QueryMethod::ExecuteAction.is_recording_method());
        assert!(!QueryMethod::SpatialQuery.is_recording_method());
    }

    #[test]
    fn recording_start_params_accept_object() {
        assert!(QueryMethod::RecordingStart.validate_params(&json!({"name": "test"})).is_ok());
    }

    #[test]
    fn recording_start_params_reject_non_object() {
        assert!(QueryMethod::RecordingStart.validate_params(&json!("bad")).is_err());
        assert!(QueryMethod::RecordingStart.validate_params(&json!(42)).is_err());
    }

    #[test]
    fn recording_stop_accepts_empty_params() {
        assert!(QueryMethod::RecordingStop.validate_params(&json!({})).is_ok());
        assert!(QueryMethod::RecordingStop.validate_params(&json!(null)).is_ok());
    }

    #[test]
    fn recording_status_accepts_empty_params() {
        assert!(QueryMethod::RecordingStatus.validate_params(&json!({})).is_ok());
    }

    #[test]
    fn recording_marker_accepts_object() {
        let params = json!({"source": "agent", "label": "checkpoint"});
        assert!(QueryMethod::RecordingMarker.validate_params(&params).is_ok());
    }

    #[test]
    fn recording_marker_rejects_non_object() {
        assert!(QueryMethod::RecordingMarker.validate_params(&json!("bad")).is_err());
    }

    // --- validate_params: get_snapshot_data ---

    #[test]
    fn snapshot_params_validate_camera_perspective() {
        let params = json!({
            "perspective": {"type": "camera"},
            "radius": 50.0,
            "include_offscreen": false,
            "detail": "standard"
        });
        assert!(QueryMethod::GetSnapshotData.validate_params(&params).is_ok());
    }

    #[test]
    fn snapshot_params_validate_point_perspective() {
        let params = json!({
            "perspective": {"type": "point", "position": [0.0, 1.0, 0.0]},
            "radius": 30.0,
            "include_offscreen": true,
            "detail": "full"
        });
        assert!(QueryMethod::GetSnapshotData.validate_params(&params).is_ok());
    }

    #[test]
    fn snapshot_params_reject_invalid_detail() {
        let params = json!({
            "perspective": {"type": "camera"},
            "radius": 50.0,
            "include_offscreen": false,
            "detail": "nonexistent"
        });
        assert!(QueryMethod::GetSnapshotData.validate_params(&params).is_err());
    }

    #[test]
    fn snapshot_params_reject_missing_required_fields() {
        let params = json!({"radius": 50.0});
        assert!(QueryMethod::GetSnapshotData.validate_params(&params).is_err());
    }

    // --- validate_params: get_frame_info ---

    #[test]
    fn frame_info_params_accept_empty_object() {
        let params = json!({});
        assert!(QueryMethod::GetFrameInfo.validate_params(&params).is_ok());
    }

    // --- validate_params: get_node_inspect ---

    #[test]
    fn inspect_params_validate() {
        let params = json!({
            "path": "TestScene3D/Player",
            "include": ["transform", "state"]
        });
        assert!(QueryMethod::GetNodeInspect.validate_params(&params).is_ok());
    }

    #[test]
    fn inspect_params_reject_unknown_category() {
        let params = json!({
            "path": "Player",
            "include": ["bogus_category"]
        });
        assert!(QueryMethod::GetNodeInspect.validate_params(&params).is_err());
    }

    // --- validate_params: get_scene_tree ---

    #[test]
    fn scene_tree_params_validate_children_action() {
        let params = json!({
            "action": "children",
            "node": "TestScene3D/Enemies"
        });
        assert!(QueryMethod::GetSceneTree.validate_params(&params).is_ok());
    }

    #[test]
    fn scene_tree_params_validate_find_action() {
        let params = json!({
            "action": "find",
            "find_by": "class",
            "find_value": "CharacterBody3D"
        });
        assert!(QueryMethod::GetSceneTree.validate_params(&params).is_ok());
    }

    // --- validate_params: execute_action ---

    #[test]
    fn action_params_validate_pause() {
        let params = json!({"action": "pause", "paused": true});
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_ok());
    }

    #[test]
    fn action_params_validate_teleport() {
        let params = json!({
            "action": "teleport",
            "path": "TestScene3D/Enemies/Scout",
            "position": [10.0, 0.0, 5.0]
        });
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_ok());
    }

    #[test]
    fn action_params_validate_set_property() {
        let params = json!({
            "action": "set_property",
            "path": "Player",
            "property": "health",
            "value": 42
        });
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_ok());
    }

    #[test]
    fn action_params_validate_call_method() {
        let params = json!({
            "action": "call_method",
            "path": "TestScene3D",
            "method": "ping",
            "args": []
        });
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_ok());
    }

    #[test]
    fn action_params_validate_advance_frames() {
        let params = json!({"action": "advance_frames", "frames": 5});
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_ok());
    }

    #[test]
    fn action_params_reject_unknown_action() {
        let params = json!({"action": "explode", "path": "Player"});
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_err());
    }

    #[test]
    fn action_params_reject_missing_action_field() {
        let params = json!({"path": "Player", "position": [0.0, 0.0, 0.0]});
        assert!(QueryMethod::ExecuteAction.validate_params(&params).is_err());
    }

    // --- validate_params: spatial_query ---

    #[test]
    fn spatial_query_params_validate_raycast() {
        let params = json!({
            "query_type": "raycast",
            "from": [0.0, 1.0, 0.0],
            "to": [5.0, 1.0, -3.0]
        });
        assert!(QueryMethod::SpatialQuery.validate_params(&params).is_ok());
    }

    #[test]
    fn spatial_query_params_validate_path_distance() {
        let params = json!({
            "query_type": "path_distance",
            "from": "Player",
            "to": "Enemy"
        });
        assert!(QueryMethod::SpatialQuery.validate_params(&params).is_ok());
    }

    #[test]
    fn spatial_query_params_reject_unknown_type() {
        let params = json!({"query_type": "warp", "from": [0.0, 0.0, 0.0]});
        assert!(QueryMethod::SpatialQuery.validate_params(&params).is_err());
    }
}
