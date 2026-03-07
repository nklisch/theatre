use serde_json::json;
use spectator_protocol::messages::Message;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::mcp::action::SpatialActionParams;
use crate::mcp::config::SpatialConfigParams;
use crate::mcp::recording::RecordingParams;
use crate::mcp::scene_tree::SceneTreeToolParams;
use crate::mcp::snapshot::SpatialSnapshotParams;
use crate::mcp::watch::SpatialWatchParams;

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
    let detail = &params.detail;
    let mut parts = vec![format!("Snapshot ({detail})")];
    if let Some(ref groups) = params.groups {
        if !groups.is_empty() {
            parts.push(format!("groups: {}", groups.join(", ")));
        }
    }
    if let Some(ref node) = params.focal_node {
        parts.push(format!("from: {node}"));
    }
    parts.join(", ")
}

pub fn action_summary(params: &SpatialActionParams) -> String {
    match params.action.as_str() {
        "pause" => {
            if params.paused.unwrap_or(true) {
                "Paused game".into()
            } else {
                "Resumed game".into()
            }
        }
        "advance_frames" => format!("Advanced {} frames", params.frames.unwrap_or(1)),
        "advance_time" => format!("Advanced {}s", params.seconds.unwrap_or(0.0)),
        "teleport" => {
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
        "set_property" => {
            let node = params.node.as_deref().unwrap_or("?");
            let prop = params.property.as_deref().unwrap_or("?");
            let val = params
                .value
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default();
            format!("Set {prop} = {val} on {node}")
        }
        "call_method" => {
            let node = params.node.as_deref().unwrap_or("?");
            let method = params.method.as_deref().unwrap_or("?");
            let args = params
                .args
                .as_ref()
                .or(params.method_args.as_ref())
                .map(|a| {
                    a.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!("Called {method}({args}) on {node}")
        }
        "emit_signal" => {
            let node = params.node.as_deref().unwrap_or("?");
            let signal = params.signal.as_deref().unwrap_or("?");
            format!("Emitted {signal} on {node}")
        }
        "spawn_node" => {
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
        "remove_node" => {
            let node = params.node.as_deref().unwrap_or("?");
            format!("Removed {node}")
        }
        other => format!("Action: {other}"),
    }
}

pub fn inspect_summary(node: &str) -> String {
    format!("Inspecting {node}")
}

pub fn scene_tree_summary(params: &SceneTreeToolParams) -> String {
    match params.action.as_str() {
        "find" => {
            let by = params.find_by.as_deref().unwrap_or("?");
            let val = params.find_value.as_deref().unwrap_or("?");
            format!("Scene tree find: {by}={val}")
        }
        "roots" => "Scene tree: roots".into(),
        "children" => format!(
            "Scene tree: children of {}",
            params.node.as_deref().unwrap_or("root")
        ),
        "subtree" => format!(
            "Scene tree: subtree of {}",
            params.node.as_deref().unwrap_or("root")
        ),
        "ancestors" => format!(
            "Scene tree: ancestors of {}",
            params.node.as_deref().unwrap_or("?")
        ),
        other => format!("Scene tree: {other}"),
    }
}

pub fn delta_summary() -> String {
    "Checking delta".into()
}

pub fn watch_summary(params: &SpatialWatchParams) -> String {
    match params.action.as_str() {
        "add" => {
            let node = params
                .watch
                .as_ref()
                .map(|w| w.node.as_str())
                .unwrap_or("?");
            format!("Watching {node}")
        }
        "remove" => format!(
            "Removed watch {}",
            params.watch_id.as_deref().unwrap_or("?")
        ),
        "list" => "Listing watches".into(),
        "clear" => "Cleared all watches".into(),
        other => format!("Watch: {other}"),
    }
}

pub fn recording_summary(params: &RecordingParams) -> String {
    match params.action.as_str() {
        "start" => {
            let name = params.recording_name.as_deref().unwrap_or("(auto)");
            format!("Started recording: {name}")
        }
        "stop" => "Stopped recording".into(),
        "status" => "Checking recording status".into(),
        "list" => "Listing recordings".into(),
        "delete" => {
            let id = params.recording_id.as_deref().unwrap_or("?");
            format!("Deleted recording {id}")
        }
        "markers" => {
            let id = params.recording_id.as_deref().unwrap_or("current");
            format!("Listing markers for {id}")
        }
        "add_marker" => {
            let label = params.marker_label.as_deref().unwrap_or("(no label)");
            format!("Added marker: {label}")
        }
        "snapshot_at" => {
            let frame_info = if let Some(f) = params.at_frame {
                format!("frame {f}")
            } else if let Some(t) = params.at_time_ms {
                format!("{t}ms")
            } else {
                "?".into()
            };
            let rec = params.recording_id.as_deref().unwrap_or("latest");
            format!("Snapshot at {frame_info} in {rec}")
        }
        "query_range" => {
            let from = params
                .from_frame
                .map(|f| f.to_string())
                .unwrap_or("?".into());
            let to = params
                .to_frame
                .map(|f| f.to_string())
                .unwrap_or("?".into());
            let node = params.node.as_deref().unwrap_or("?");
            format!("Query range {from}-{to} for {node}")
        }
        "diff_frames" => {
            let a = params
                .frame_a
                .map(|f| f.to_string())
                .unwrap_or("?".into());
            let b = params
                .frame_b
                .map(|f| f.to_string())
                .unwrap_or("?".into());
            format!("Diff frames {a} vs {b}")
        }
        "find_event" => {
            let evt = params.event_type.as_deref().unwrap_or("?");
            let filter = params.event_filter.as_deref().unwrap_or("");
            if filter.is_empty() {
                format!("Find events: {evt}")
            } else {
                format!("Find events: {evt} filter={filter}")
            }
        }
        other => format!("Recording: {other}"),
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
