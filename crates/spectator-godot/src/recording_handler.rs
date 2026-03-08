use godot::obj::Gd;
use godot::prelude::*;
use serde_json::{Value, json};

use crate::recorder::SpectatorRecorder;

/// Handle recording-related TCP queries dispatched from the TCP server.
pub fn handle_recording_query(
    recorder: &mut Gd<SpectatorRecorder>,
    method: &str,
    params: &Value,
) -> Result<Value, (String, String)> {
    match method {
        "recording_start" => handle_start(recorder, params),
        "recording_stop" => handle_stop(recorder),
        "recording_status" => handle_status(recorder),
        "recording_list" => handle_list(recorder, params),
        "recording_delete" => handle_delete(recorder, params),
        "recording_marker" => handle_marker(recorder, params),
        "recording_markers" => handle_get_markers(recorder, params),
        "recording_resolve_path" => handle_resolve_path(params),
        _ => Err((
            "method_not_found".into(),
            format!("Unknown recording method: {method}"),
        )),
    }
}

fn handle_resolve_path(_params: &Value) -> Result<Value, (String, String)> {
    let storage = "user://spectator_recordings/";
    let globalized = crate::recorder::globalize_path(storage);
    Ok(json!({ "path": globalized }))
}

fn handle_start(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    if recorder.bind().is_recording() {
        return Err(("recording_active".into(), "A recording is already active".into()));
    }

    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let storage_path = params
        .get("storage_path")
        .and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");
    let interval = params
        .get("capture_interval")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;
    let max_frames = params
        .get("max_frames")
        .and_then(|v| v.as_u64())
        .unwrap_or(36000) as u32;

    let id = recorder.bind_mut().start_recording(
        name.into(),
        storage_path.into(),
        interval,
        max_frames,
    );

    if id.is_empty() {
        return Err(("internal_error".into(), "Failed to start recording".into()));
    }

    let name_val = recorder.bind().get_recording_name().to_string();
    let started_at_frame = godot::classes::Engine::singleton().get_physics_frames();
    Ok(json!({
        "recording_id": id.to_string(),
        "name": name_val,
        "started_at_frame": started_at_frame,
    }))
}

fn handle_stop(recorder: &mut Gd<SpectatorRecorder>) -> Result<Value, (String, String)> {
    if !recorder.bind().is_recording() {
        return Err(("no_recording_active".into(), "No recording is active".into()));
    }

    let meta = recorder.bind_mut().stop_recording();

    Ok(json!({
        "recording_id": meta.get("recording_id").map(|v| v.to_string()).unwrap_or_default(),
        "name": meta.get("name").map(|v| v.to_string()).unwrap_or_default(),
        "frames_captured": meta.get("frames_captured").map(|v: godot::builtin::Variant| v.to::<u32>()).unwrap_or(0),
        "duration_ms": meta.get("duration_ms").map(|v: godot::builtin::Variant| v.to::<u64>()).unwrap_or(0),
        "frame_range": [
            meta.get("started_at_frame").map(|v: godot::builtin::Variant| v.to::<u64>()).unwrap_or(0),
            meta.get("ended_at_frame").map(|v: godot::builtin::Variant| v.to::<u64>()).unwrap_or(0),
        ],
    }))
}

fn handle_status(recorder: &mut Gd<SpectatorRecorder>) -> Result<Value, (String, String)> {
    let rec = recorder.bind();
    Ok(json!({
        "recording_active": rec.is_recording(),
        "recording_id": rec.get_recording_id().to_string(),
        "name": rec.get_recording_name().to_string(),
        "frames_captured": rec.get_frames_captured(),
        "duration_ms": rec.get_elapsed_ms(),
        "buffer_size_kb": rec.get_buffer_size_kb(),
    }))
}

fn handle_list(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let storage_path = params
        .get("storage_path")
        .and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");

    let recordings = recorder.bind().list_recordings(storage_path.into());

    let list: Vec<Value> = recordings
        .iter_shared()
        .map(|dict| {
            json!({
                "recording_id": dict.get("id").map(|v| v.to_string()).unwrap_or_default(),
                "name": dict.get("name").map(|v| v.to_string()).unwrap_or_default(),
                "frames_captured": dict.get("frames_captured").map(|v: godot::builtin::Variant| v.to::<u32>()).unwrap_or(0),
                "duration_ms": dict.get("duration_ms").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                "frame_range": [
                    dict.get("frame_range_start").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                    dict.get("frame_range_end").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                ],
                "markers_count": dict.get("markers_count").map(|v: godot::builtin::Variant| v.to::<u32>()).unwrap_or(0),
                "size_kb": dict.get("size_kb").map(|v: godot::builtin::Variant| v.to::<u32>()).unwrap_or(0),
                "created_at_ms": dict.get("created_at_ms").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
            })
        })
        .collect();

    Ok(json!({ "recordings": list }))
}

fn handle_delete(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let id = params
        .get("recording_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ("invalid_params".into(), "recording_id is required".to_string()))?;
    let storage_path = params
        .get("storage_path")
        .and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");

    let ok = recorder.bind().delete_recording(storage_path.into(), id.into());

    if ok {
        Ok(json!({ "result": "ok", "recording_id": id }))
    } else {
        Err((
            "recording_not_found".into(),
            format!("Recording '{id}' not found"),
        ))
    }
}

fn handle_marker(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    if !recorder.bind().is_recording() {
        return Err((
            "no_recording_active".into(),
            "No recording is active to add a marker to".into(),
        ));
    }

    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("");

    recorder.bind_mut().add_marker(source.into(), label.into());

    let frame = godot::classes::Engine::singleton().get_physics_frames();
    Ok(json!({
        "result": "ok",
        "frame": frame,
        "source": source,
        "label": label,
    }))
}

fn handle_get_markers(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let id = params
        .get("recording_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ("invalid_params".into(), "recording_id is required".to_string()))?;
    let storage_path = params
        .get("storage_path")
        .and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");

    let markers = recorder
        .bind()
        .get_recording_markers(storage_path.into(), id.into());

    let list: Vec<Value> = markers
        .iter_shared()
        .map(|dict| {
            json!({
                "frame": dict.get("frame").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                "timestamp_ms": dict.get("timestamp_ms").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                "source": dict.get("source").map(|v| v.to_string()).unwrap_or_default(),
                "label": dict.get("label").map(|v| v.to_string()).unwrap_or_default(),
            })
        })
        .collect();

    Ok(json!({ "recording_id": id, "markers": list }))
}
