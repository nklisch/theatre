use serde_json::json;
use stage_protocol::messages::Message;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::mcp::action::{ActionType, SpatialActionParams};
use crate::mcp::clips::{ClipAction, ClipsParams};
use crate::mcp::config::SpatialConfigParams;
use crate::mcp::scene_tree::SceneTreeToolParams;
use crate::mcp::snapshot::SpatialSnapshotParams;
use crate::mcp::watch::{SpatialWatchParams, WatchAction};
use stage_protocol::query::SceneTreeAction;

/// Build an activity_log Event message to push to the addon.
///
/// `meta` is an optional JSON object included as a top-level `"meta"` field.
/// Watch events use `meta: { "active_watches": N }` so the dock can display
/// the authoritative watch count without parsing summary strings.
pub fn build_activity_message(
    entry_type: &str,
    summary: &str,
    tool: &str,
    meta: Option<serde_json::Value>,
) -> Message {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let mut data = json!({
        "entry_type": entry_type,
        "summary": summary,
        "tool": tool,
        "timestamp": timestamp,
    });
    if let (Some(m), Some(obj)) = (meta, data.as_object_mut()) {
        obj.insert("meta".into(), m);
    }
    Message::Event {
        event: "activity_log".to_string(),
        data,
    }
}

pub fn snapshot_summary(params: &SpatialSnapshotParams) -> String {
    if let Some(ref cluster) = params.expand {
        return format!("Expanding cluster: {cluster}");
    }
    let detail = format!("{:?}", params.detail).to_lowercase();
    let mut parts = vec![format!("Snapshot ({detail})")];
    if let Some(ref groups) = params.groups
        && !groups.is_empty()
    {
        parts.push(format!("groups: {}", groups.join(", ")));
    }
    if let Some(ref node) = params.focal_node {
        parts.push(format!("from: {node}"));
    }
    parts.join(", ")
}

pub fn action_summary(params: &SpatialActionParams) -> String {
    match &params.action {
        ActionType::Pause => {
            if params.paused.unwrap_or(true) {
                "Paused game".into()
            } else {
                "Resumed game".into()
            }
        }
        ActionType::AdvanceFrames => format!("Advanced {} frames", params.frames.unwrap_or(1)),
        ActionType::AdvanceTime => format!("Advanced {}s", params.seconds.unwrap_or(0.0)),
        ActionType::Teleport => {
            let node = params.node.as_deref().unwrap_or("?");
            let pos = params
                .position
                .as_ref()
                .map(|p| {
                    let strs: Vec<_> = p.iter().map(|v| format!("{v:.1}")).collect();
                    format!("[{}]", strs.join(", "))
                })
                .unwrap_or_default();
            format!("Teleported {node} → {pos}")
        }
        ActionType::SetProperty => {
            let node = params.node.as_deref().unwrap_or("?");
            let prop = params.property.as_deref().unwrap_or("?");
            let val = params
                .value
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default();
            format!("Set {prop} = {val} on {node}")
        }
        ActionType::CallMethod => {
            let node = params.node.as_deref().unwrap_or("?");
            let method = params.method.as_deref().unwrap_or("?");
            let args = params.args.as_ref().map(|a| {
                    a.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!("Called {method}({args}) on {node}")
        }
        ActionType::EmitSignal => {
            let node = params.node.as_deref().unwrap_or("?");
            let signal = params.signal.as_deref().unwrap_or("?");
            format!("Emitted {signal} on {node}")
        }
        ActionType::SpawnNode => {
            let scene = params
                .scene_path
                .as_deref()
                .unwrap_or("?")
                .rsplit('/')
                .next()
                .unwrap_or("?");
            let name = params.name.as_deref().unwrap_or("?");
            format!("Spawned {scene} as {name}")
        }
        ActionType::RemoveNode => {
            let node = params.node.as_deref().unwrap_or("?");
            format!("Removed {node}")
        }
        ActionType::ActionPress => {
            let action = params.input_action.as_deref().unwrap_or("?");
            let strength = params.strength.unwrap_or(1.0);
            format!("Pressed action '{action}' (strength {strength:.2})")
        }
        ActionType::ActionRelease => {
            let action = params.input_action.as_deref().unwrap_or("?");
            format!("Released action '{action}'")
        }
        ActionType::InjectKey => {
            let key = params.keycode.as_deref().unwrap_or("?");
            let state = if params.pressed.unwrap_or(true) {
                "press"
            } else {
                "release"
            };
            format!("Key {state}: {key}")
        }
        ActionType::InjectMouseButton => {
            let btn = params.button.as_deref().unwrap_or("?");
            let state = if params.pressed.unwrap_or(true) {
                "press"
            } else {
                "release"
            };
            format!("Mouse {state}: {btn}")
        }
    }
}

pub fn inspect_summary(node: &str) -> String {
    format!("Inspecting {node}")
}

pub fn scene_tree_summary(params: &SceneTreeToolParams) -> String {
    match params.action {
        SceneTreeAction::Find => {
            let by = params
                .find_by
                .as_ref()
                .map(|f| format!("{f:?}").to_lowercase())
                .unwrap_or_else(|| "?".into());
            let val = params.find_value.as_deref().unwrap_or("?");
            format!("Scene tree find: {by}={val}")
        }
        SceneTreeAction::Roots => "Scene tree: roots".into(),
        SceneTreeAction::Children => format!(
            "Scene tree: children of {}",
            params.node.as_deref().unwrap_or("root")
        ),
        SceneTreeAction::Subtree => format!(
            "Scene tree: subtree of {}",
            params.node.as_deref().unwrap_or("root")
        ),
        SceneTreeAction::Ancestors => format!(
            "Scene tree: ancestors of {}",
            params.node.as_deref().unwrap_or("?")
        ),
    }
}

pub fn delta_summary() -> String {
    "Checking delta".into()
}

pub fn watch_summary(params: &SpatialWatchParams) -> String {
    match &params.action {
        WatchAction::Add => {
            let node = params
                .watch
                .as_ref()
                .map(|w| w.node.as_str())
                .unwrap_or("?");
            format!("Watching {node}")
        }
        WatchAction::Remove => format!(
            "Removed watch {}",
            params.watch_id.as_deref().unwrap_or("?")
        ),
        WatchAction::List => "Listing watches".into(),
        WatchAction::Clear => "Cleared all watches".into(),
    }
}

pub fn clips_summary(params: &ClipsParams) -> String {
    match &params.action {
        ClipAction::AddMarker => {
            let label = params.marker_label.as_deref().unwrap_or("(no label)");
            format!("Marker: {label}")
        }
        ClipAction::Save => {
            let label = params.marker_label.as_deref().unwrap_or("agent save");
            format!("Saved clip: {label}")
        }
        ClipAction::Status => "Dashcam status".into(),
        ClipAction::List => "Listing clips".into(),
        ClipAction::Delete => {
            let id = params.clip_id.as_deref().unwrap_or("?");
            format!("Deleted clip {id}")
        }
        ClipAction::Markers => {
            let id = params.clip_id.as_deref().unwrap_or("latest");
            format!("Markers for {id}")
        }
        ClipAction::SnapshotAt => {
            let frame_info = if let Some(f) = params.at_frame {
                format!("frame {f}")
            } else if let Some(t) = params.at_time_ms {
                format!("{t}ms")
            } else {
                "?".into()
            };
            let clip = params.clip_id.as_deref().unwrap_or("latest");
            format!("Snapshot at {frame_info} in {clip}")
        }
        ClipAction::QueryRange => {
            let from = params
                .from_frame
                .map(|f| f.to_string())
                .unwrap_or("?".into());
            let to = params.to_frame.map(|f| f.to_string()).unwrap_or("?".into());
            let node = params.node.as_deref().unwrap_or("?");
            format!("Query range {from}-{to} for {node}")
        }
        ClipAction::DiffFrames => {
            let a = params.frame_a.map(|f| f.to_string()).unwrap_or("?".into());
            let b = params.frame_b.map(|f| f.to_string()).unwrap_or("?".into());
            format!("Diff frames {a} vs {b}")
        }
        ClipAction::FindEvent => {
            let evt = params.event_type.as_deref().unwrap_or("?");
            let filter = params.event_filter.as_deref().unwrap_or("");
            if filter.is_empty() {
                format!("Find events: {evt}")
            } else {
                format!("Find events: {evt} filter={filter}")
            }
        }
        ClipAction::Trajectory => {
            let node = params.node.as_deref().unwrap_or("?");
            format!("Trajectory for {node}")
        }
        ClipAction::ScreenshotAt => "Screenshot at frame".into(),
        ClipAction::Screenshots => "List screenshots".into(),
    }
}

pub fn config_summary(params: &SpatialConfigParams) -> String {
    let mut keys: Vec<&str> = Vec::new();
    if params.static_patterns.is_some() {
        keys.push("static_patterns");
    }
    if params.state_properties.is_some() {
        keys.push("state_properties");
    }
    if params.cluster_by.is_some() {
        keys.push("cluster_by");
    }
    if params.bearing_format.is_some() {
        keys.push("bearing_format");
    }
    if params.expose_internals.is_some() {
        keys.push("expose_internals");
    }
    if params.poll_interval.is_some() {
        keys.push("poll_interval");
    }
    if params.token_hard_cap.is_some() {
        keys.push("token_hard_cap");
    }
    if keys.is_empty() {
        "Config: view current".into()
    } else {
        format!("Config: {}", keys.join(", "))
    }
}
