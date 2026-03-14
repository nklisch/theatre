use crate::query::{
    ActionRequest, GetFrameInfoParams, GetNodeInspectParams, GetSceneTreeParams,
    GetSnapshotDataParams, SpatialQueryRequest,
};

/// Known query method names dispatched by the addon's TCP query handler.
///
/// Used to validate method names and deserialize params before reaching Godot.
/// This is the authoritative list — adding a new method requires updating this enum.
///
/// Methods prefixed `Recording*` are clip management methods routed through
/// `recording_handler.rs` rather than `query_handler.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMethod {
    // Spatial query methods (routed through query_handler.rs)
    GetSnapshotData,
    GetFrameInfo,
    GetNodeInspect,
    GetSceneTree,
    ExecuteAction,
    SpatialQuery,
    // Clip management methods (routed through recording_handler.rs)
    RecordingMarker,
    RecordingMarkers,
    RecordingList,
    RecordingDelete,
    RecordingResolvePath,
    DashcamStatus,
    DashcamFlush,
    DashcamConfig,
}

impl QueryMethod {
    /// Resolve a wire method name to its enum variant. Returns `None` for unknown methods.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "get_snapshot_data" => Some(Self::GetSnapshotData),
            "get_frame_info" => Some(Self::GetFrameInfo),
            "get_node_inspect" => Some(Self::GetNodeInspect),
            "get_scene_tree" => Some(Self::GetSceneTree),
            "execute_action" => Some(Self::ExecuteAction),
            "spatial_query" => Some(Self::SpatialQuery),
            "recording_marker" => Some(Self::RecordingMarker),
            "recording_markers" => Some(Self::RecordingMarkers),
            "recording_list" => Some(Self::RecordingList),
            "recording_delete" => Some(Self::RecordingDelete),
            "recording_resolve_path" => Some(Self::RecordingResolvePath),
            "dashcam_status" => Some(Self::DashcamStatus),
            "dashcam_flush" => Some(Self::DashcamFlush),
            "dashcam_config" => Some(Self::DashcamConfig),
            _ => None,
        }
    }

    /// Wire name for this method (inverse of `parse`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GetSnapshotData => "get_snapshot_data",
            Self::GetFrameInfo => "get_frame_info",
            Self::GetNodeInspect => "get_node_inspect",
            Self::GetSceneTree => "get_scene_tree",
            Self::ExecuteAction => "execute_action",
            Self::SpatialQuery => "spatial_query",
            Self::RecordingMarker => "recording_marker",
            Self::RecordingMarkers => "recording_markers",
            Self::RecordingList => "recording_list",
            Self::RecordingDelete => "recording_delete",
            Self::RecordingResolvePath => "recording_resolve_path",
            Self::DashcamStatus => "dashcam_status",
            Self::DashcamFlush => "dashcam_flush",
            Self::DashcamConfig => "dashcam_config",
        }
    }

    /// Returns true if this method is routed through `recording_handler.rs`.
    pub fn is_clip_method(self) -> bool {
        matches!(
            self,
            Self::RecordingMarker
                | Self::RecordingMarkers
                | Self::RecordingList
                | Self::RecordingDelete
                | Self::RecordingResolvePath
                | Self::DashcamStatus
                | Self::DashcamFlush
                | Self::DashcamConfig
        )
    }

    /// Validate that `params` deserialize correctly for this method.
    /// Returns `Ok(())` or a human-readable error string.
    ///
    /// Clip methods use ad-hoc JSON parsing in `recording_handler.rs` rather
    /// than typed structs, so only minimal structural validation is done for them.
    pub fn validate_params(self, params: &serde_json::Value) -> Result<(), String> {
        match self {
            Self::GetSnapshotData => {
                serde_json::from_value::<GetSnapshotDataParams>(params.clone())
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            Self::GetFrameInfo => serde_json::from_value::<GetFrameInfoParams>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::GetNodeInspect => serde_json::from_value::<GetNodeInspectParams>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::GetSceneTree => serde_json::from_value::<GetSceneTreeParams>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::ExecuteAction => serde_json::from_value::<ActionRequest>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::SpatialQuery => serde_json::from_value::<SpatialQueryRequest>(params.clone())
                .map(|_| ())
                .map_err(|e| e.to_string()),
            // Clip methods: must be a JSON object (handler does field-level parsing)
            Self::RecordingMarker => {
                if !params.is_object() {
                    return Err("recording_marker params must be an object".into());
                }
                Ok(())
            }
            Self::RecordingResolvePath
            | Self::DashcamStatus
            | Self::DashcamFlush
            | Self::DashcamConfig => Ok(()), // no required params or ad-hoc object
            Self::RecordingMarkers | Self::RecordingList | Self::RecordingDelete => {
                if !params.is_null() && !params.is_object() {
                    return Err(format!(
                        "{} params must be an object or null",
                        self.as_str()
                    ));
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

    // --- parse ---

    #[test]
    fn all_known_methods_resolve() {
        let cases = [
            ("get_snapshot_data", QueryMethod::GetSnapshotData),
            ("get_frame_info", QueryMethod::GetFrameInfo),
            ("get_node_inspect", QueryMethod::GetNodeInspect),
            ("get_scene_tree", QueryMethod::GetSceneTree),
            ("execute_action", QueryMethod::ExecuteAction),
            ("spatial_query", QueryMethod::SpatialQuery),
            ("recording_marker", QueryMethod::RecordingMarker),
            ("recording_markers", QueryMethod::RecordingMarkers),
            ("recording_list", QueryMethod::RecordingList),
            ("recording_delete", QueryMethod::RecordingDelete),
            ("recording_resolve_path", QueryMethod::RecordingResolvePath),
            ("dashcam_status", QueryMethod::DashcamStatus),
            ("dashcam_flush", QueryMethod::DashcamFlush),
            ("dashcam_config", QueryMethod::DashcamConfig),
        ];
        for (name, expected) in cases {
            assert_eq!(
                QueryMethod::parse(name),
                Some(expected),
                "failed for {name}"
            );
        }
    }

    #[test]
    fn unknown_method_returns_none() {
        assert_eq!(QueryMethod::parse("bogus"), None);
        assert_eq!(QueryMethod::parse("get_snapshot"), None);
        assert_eq!(QueryMethod::parse(""), None);
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
            QueryMethod::RecordingMarker,
            QueryMethod::RecordingMarkers,
            QueryMethod::RecordingList,
            QueryMethod::RecordingDelete,
            QueryMethod::RecordingResolvePath,
            QueryMethod::DashcamStatus,
            QueryMethod::DashcamFlush,
            QueryMethod::DashcamConfig,
        ];
        for method in methods {
            let name = method.as_str();
            assert_eq!(
                QueryMethod::parse(name),
                Some(method),
                "round-trip failed for {name}"
            );
        }
    }

    // --- clip method dispatch ---

    #[test]
    fn clip_methods_are_flagged_correctly() {
        assert!(QueryMethod::RecordingMarker.is_clip_method());
        assert!(QueryMethod::RecordingMarkers.is_clip_method());
        assert!(QueryMethod::RecordingList.is_clip_method());
        assert!(QueryMethod::RecordingDelete.is_clip_method());
        assert!(QueryMethod::RecordingResolvePath.is_clip_method());
        assert!(QueryMethod::DashcamStatus.is_clip_method());
        assert!(QueryMethod::DashcamFlush.is_clip_method());
        assert!(QueryMethod::DashcamConfig.is_clip_method());
    }

    #[test]
    fn spatial_methods_are_not_clip_methods() {
        assert!(!QueryMethod::GetSnapshotData.is_clip_method());
        assert!(!QueryMethod::ExecuteAction.is_clip_method());
        assert!(!QueryMethod::SpatialQuery.is_clip_method());
    }

    #[test]
    fn recording_start_is_unknown_method() {
        assert_eq!(QueryMethod::parse("recording_start"), None);
        assert_eq!(QueryMethod::parse("recording_stop"), None);
        assert_eq!(QueryMethod::parse("recording_status"), None);
    }

    #[test]
    fn recording_marker_accepts_object() {
        let params = json!({"source": "agent", "label": "checkpoint"});
        assert!(
            QueryMethod::RecordingMarker
                .validate_params(&params)
                .is_ok()
        );
    }

    #[test]
    fn recording_marker_rejects_non_object() {
        assert!(
            QueryMethod::RecordingMarker
                .validate_params(&json!("bad"))
                .is_err()
        );
    }

    #[test]
    fn dashcam_methods_accept_empty_params() {
        assert!(
            QueryMethod::DashcamStatus
                .validate_params(&json!({}))
                .is_ok()
        );
        assert!(
            QueryMethod::DashcamFlush
                .validate_params(&json!({}))
                .is_ok()
        );
        assert!(
            QueryMethod::DashcamConfig
                .validate_params(&json!({}))
                .is_ok()
        );
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
        assert!(
            QueryMethod::GetSnapshotData
                .validate_params(&params)
                .is_ok()
        );
    }

    #[test]
    fn snapshot_params_validate_point_perspective() {
        let params = json!({
            "perspective": {"type": "point", "position": [0.0, 1.0, 0.0]},
            "radius": 30.0,
            "include_offscreen": true,
            "detail": "full"
        });
        assert!(
            QueryMethod::GetSnapshotData
                .validate_params(&params)
                .is_ok()
        );
    }

    #[test]
    fn snapshot_params_reject_invalid_detail() {
        let params = json!({
            "perspective": {"type": "camera"},
            "radius": 50.0,
            "include_offscreen": false,
            "detail": "nonexistent"
        });
        assert!(
            QueryMethod::GetSnapshotData
                .validate_params(&params)
                .is_err()
        );
    }

    #[test]
    fn snapshot_params_reject_missing_required_fields() {
        let params = json!({"radius": 50.0});
        assert!(
            QueryMethod::GetSnapshotData
                .validate_params(&params)
                .is_err()
        );
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
        assert!(
            QueryMethod::GetNodeInspect
                .validate_params(&params)
                .is_err()
        );
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
