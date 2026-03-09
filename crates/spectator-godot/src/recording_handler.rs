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
        "recording_list" => handle_list(recorder, params),
        "recording_delete" => handle_delete(recorder, params),
        "recording_marker" => handle_marker(recorder, params),
        "recording_markers" => handle_get_markers(recorder, params),
        "recording_resolve_path" => handle_resolve_path(params),
        "dashcam_status" => handle_dashcam_status(recorder),
        "dashcam_flush" => handle_dashcam_flush(recorder, params),
        "dashcam_config" => handle_dashcam_config(recorder, params),
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

fn unix_ms_to_iso8601(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms / 1000;
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    // Compute year/month/day from days_since_epoch (Unix epoch = 1970-01-01)
    let mut days = days_since_epoch;
    let mut year = 1970i64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1i64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days + 1;

    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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
            let created_at_unix_ms = dict
                .get("created_at_unix_ms")
                .map(|v: godot::builtin::Variant| v.to::<i64>())
                .unwrap_or(0);
            let created_at = unix_ms_to_iso8601(created_at_unix_ms);

            let trigger_label: Option<String> = dict
                .get("trigger_label")
                .map(|v| v.to_string())
                .filter(|s| !s.is_empty());

            let capture: Option<Value> = dict
                .get("capture_json")
                .map(|v| v.to_string())
                .and_then(|s| serde_json::from_str(&s).ok());

            let mut entry = json!({
                "clip_id": dict.get("clip_id").map(|v| v.to_string()).unwrap_or_default(),
                "name": dict.get("name").map(|v| v.to_string()).unwrap_or_default(),
                "frames_captured": dict.get("frames_captured").map(|v: godot::builtin::Variant| v.to::<u32>()).unwrap_or(0),
                "duration_ms": dict.get("duration_ms").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                "frame_range": [
                    dict.get("frame_range_start").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                    dict.get("frame_range_end").map(|v: godot::builtin::Variant| v.to::<i64>()).unwrap_or(0),
                ],
                "markers_count": dict.get("markers_count").map(|v: godot::builtin::Variant| v.to::<u32>()).unwrap_or(0),
                "size_kb": dict.get("size_kb").map(|v: godot::builtin::Variant| v.to::<u32>()).unwrap_or(0),
                "created_at": created_at,
                "dashcam": dict.get("dashcam").map(|v: godot::builtin::Variant| v.to::<bool>()).unwrap_or(false),
                "tier": dict.get("dashcam_tier").map(|v| v.to_string()).unwrap_or_default(),
            });

            if let Some(label) = trigger_label {
                entry["trigger_label"] = json!(label);
            }
            if let Some(cap) = capture {
                entry["capture"] = cap;
            }

            entry
        })
        .collect();

    Ok(json!({ "clips": list }))
}

fn handle_delete(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let id = params
        .get("clip_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ("invalid_params".into(), "clip_id is required".to_string()))?;
    let storage_path = params
        .get("storage_path")
        .and_then(|v| v.as_str())
        .unwrap_or("user://spectator_recordings/");

    let ok = recorder
        .bind()
        .delete_recording(storage_path.into(), id.into());

    if ok {
        Ok(json!({ "result": "ok", "clip_id": id }))
    } else {
        Err(("clip_not_found".into(), format!("Clip '{id}' not found")))
    }
}

fn handle_marker(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let dashcam_active = {
        let state = recorder.bind().get_dashcam_state().to_string();
        state == "buffering" || state == "post_capture"
    };

    if !dashcam_active {
        return Err((
            "no_dashcam_active".into(),
            "Dashcam is not active to add a marker to".into(),
        ));
    }

    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("");

    let tier = if source == "agent" || source == "human" {
        "deliberate"
    } else {
        "system"
    };
    recorder
        .bind_mut()
        .trigger_dashcam_clip(source.into(), label.into(), tier.into());

    let frame = godot::classes::Engine::singleton().get_physics_frames();
    Ok(json!({
        "ok": true,
        "frame": frame,
        "source": source,
        "label": label,
        "dashcam_triggered": true,
        "dashcam_tier": tier,
    }))
}

fn handle_get_markers(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let id = params
        .get("clip_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ("invalid_params".into(), "clip_id is required".to_string()))?;
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

    Ok(json!({ "clip_id": id, "markers": list }))
}

fn handle_dashcam_status(recorder: &mut Gd<SpectatorRecorder>) -> Result<Value, (String, String)> {
    let rec = recorder.bind();
    let state_str = rec.get_dashcam_state().to_string();
    let buffer_frames = rec.get_dashcam_buffer_frames();
    let buffer_kb = rec.get_dashcam_buffer_kb();
    let dashcam_enabled =
        rec.is_dashcam_active() || state_str == "buffering" || state_str == "post_capture";
    let config_json_str = rec.get_dashcam_config_json().to_string();
    drop(rec);

    let config: Value = serde_json::from_str(&config_json_str).unwrap_or(json!({}));

    Ok(json!({
        "dashcam_enabled": dashcam_enabled,
        "state": state_str,
        "buffer_frames": buffer_frames,
        "buffer_kb": buffer_kb,
        "config": config,
    }))
}

fn handle_dashcam_flush(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let label = params
        .get("marker_label")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Capture buffer frame count before flush (flush drains the buffer)
    let buffer_frames = recorder.bind().get_dashcam_buffer_frames();

    let clip_id = recorder
        .bind_mut()
        .flush_dashcam_clip(label.into())
        .to_string();

    if clip_id.is_empty() {
        Err((
            "dashcam_not_active".into(),
            "Dashcam is not active or flush failed".into(),
        ))
    } else {
        Ok(json!({
            "result": "ok",
            "clip_id": clip_id,
            "tier": "deliberate",
            "frames": buffer_frames,
        }))
    }
}

fn handle_dashcam_config(
    recorder: &mut Gd<SpectatorRecorder>,
    params: &Value,
) -> Result<Value, (String, String)> {
    let config_json = params.to_string();
    let ok = recorder
        .bind_mut()
        .apply_dashcam_config(config_json.as_str().into());
    if ok {
        Ok(json!({ "result": "ok" }))
    } else {
        Err((
            "invalid_params".into(),
            "Failed to apply dashcam config".into(),
        ))
    }
}
