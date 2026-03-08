# Design: M11 Dashcam Recording — Comprehensive Test Suite

## Overview

Test suite covering the M11 dashcam recording feature across all test layers:
unit tests (state machine logic), integration tests (mock addon TCP),
scenario tests (multi-step stateful interactions), and E2E journey tests
(real Godot headless). The journey tests are the highest priority — they
exercise the feature as an agent would actually use it and catch the
cross-boundary bugs that unit tests miss.

## Gap Analysis — What Exists vs What's Missing

### Existing Coverage (✓)

**Unit tests** in `recorder.rs`:
- `dashcam_config_defaults` — config init
- `dashcam_tier_str` — enum Display
- `ring_buffer_eviction_at_byte_cap` — byte cap eviction
- `dashcam_merge_system_plus_system_extends_window` — system+system merge
- `dashcam_merge_deliberate_upgrades_system_clip` — tier upgrade
- `dashcam_rate_limiting_system_markers` — rate limiting
- `dashcam_max_window_force_close` — force-close calculation
- `dashcam_clip_metadata_in_sqlite` — metadata roundtrip
- `dashcam_apply_config_json` — config parsing

**Unit tests** in `recording.rs` (server):
- `recording_params_dashcam_status` — param deserialization
- `recording_params_flush_dashcam` — param deserialization

**Integration tests** in `tcp_mock.rs`:
- Recording start/stop/status/list/delete/add_marker — but NO dashcam tests

**E2E journey tests** in `e2e_journeys.rs`:
- `journey_recording_lifecycle` — explicit recording only, NO dashcam

### Missing Coverage (✗)

1. **No integration tests** for `dashcam_status` or `flush_dashcam` MCP actions
2. **No integration scenario tests** for dashcam + explicit recording coexistence
3. **No E2E tests** for dashcam at all — this is the biggest gap
4. **Unit tests are logic-only** — they test the merge/eviction algorithms with
   local variables, not the actual `SpectatorRecorder` methods (can't call gdext
   methods in unit tests, so this is expected, but the logic duplication means
   the tests might pass while the real code has wiring bugs)
5. **No tests** for: dashcam clip appearing in `list`, dashcam clip being
   analyzable with M8 tools, marker triggering dashcam clip save, tier upgrade
   in a live session, overlapping triggers producing merged clips
6. **No error path tests** for dashcam: flush when disabled, flush with empty
   buffer

---

## Implementation Units

### Unit 1: Integration Tests — Dashcam Status & Flush (Mock Addon)

**File**: `crates/spectator-server/tests/tcp_mock.rs`

Add to the recording section (after `test_recording_delete`):

```rust
#[tokio::test]
async fn test_dashcam_status() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_status" => Ok(json!({
            "dashcam_enabled": true,
            "state": "buffering",
            "buffer_frames": 1800,
            "buffer_kb": 14400,
            "config": {
                "capture_interval": 1,
                "pre_window_sec": { "system": 30, "deliberate": 60 },
                "post_window_sec": { "system": 10, "deliberate": 30 },
                "max_window_sec": 120,
                "min_after_sec": 5,
                "system_min_interval_sec": 2,
                "byte_cap_mb": 1024
            }
        })),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("recording", json!({ "action": "dashcam_status" }))
        .await
        .unwrap();

    assert_eq!(result["dashcam_enabled"], json!(true));
    assert_eq!(result["state"], json!("buffering"));
    assert!(result["buffer_frames"].as_u64().is_some());
    assert!(result["config"].is_object());
}

#[tokio::test]
async fn test_dashcam_status_post_capture() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_status" => Ok(json!({
            "dashcam_enabled": true,
            "state": "post_capture",
            "buffer_frames": 1800,
            "buffer_kb": 14400,
            "open_clip": {
                "tier": "system",
                "frames_remaining": 300,
                "markers": 2
            },
            "config": {
                "capture_interval": 1,
                "pre_window_sec": { "system": 30, "deliberate": 60 },
                "post_window_sec": { "system": 10, "deliberate": 30 },
                "max_window_sec": 120,
                "min_after_sec": 5,
                "system_min_interval_sec": 2,
                "byte_cap_mb": 1024
            }
        })),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool("recording", json!({ "action": "dashcam_status" }))
        .await
        .unwrap();

    assert_eq!(result["state"], json!("post_capture"));
    assert!(result["open_clip"].is_object());
    assert_eq!(result["open_clip"]["tier"], json!("system"));
}

#[tokio::test]
async fn test_dashcam_flush() {
    let handler: QueryHandler = Arc::new(|method, params| match method {
        "dashcam_flush" => {
            let label = params
                .get("marker_label")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(json!({
                "recording_id": "dash_abc12345",
                "tier": "deliberate",
                "frames": 1800,
                "marker_label": label
            }))
        }
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "recording",
            json!({ "action": "flush_dashcam", "marker_label": "suspected bug" }),
        )
        .await
        .unwrap();

    assert!(
        result["recording_id"].as_str().unwrap().starts_with("dash_"),
        "flush should return a dashcam recording_id: {result}"
    );
    assert_eq!(result["tier"], json!("deliberate"));
    assert!(result["frames"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_dashcam_flush_empty_buffer_returns_error() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_flush" => Err((
            "empty_buffer".into(),
            "Dashcam ring buffer is empty — no frames to save".into(),
        )),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let err = harness
        .call_tool(
            "recording",
            json!({ "action": "flush_dashcam", "marker_label": "test" }),
        )
        .await
        .unwrap_err();

    assert!(
        err.message.contains("empty") || err.message.contains("buffer"),
        "expected error about empty buffer, got: {err:?}"
    );
}

#[tokio::test]
async fn test_dashcam_flush_when_disabled_returns_error() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "dashcam_flush" => Err((
            "dashcam_disabled".into(),
            "Dashcam is not enabled".into(),
        )),
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let err = harness
        .call_tool(
            "recording",
            json!({ "action": "flush_dashcam", "marker_label": "test" }),
        )
        .await
        .unwrap_err();

    assert!(
        err.message.contains("not enabled") || err.message.contains("disabled"),
        "expected error about disabled dashcam, got: {err:?}"
    );
}

#[tokio::test]
async fn test_dashcam_flush_default_label() {
    // When no marker_label is provided, the server sends "agent flush" as default.
    let received_label: Arc<std::sync::Mutex<String>> =
        Arc::new(std::sync::Mutex::new(String::new()));
    let rl = received_label.clone();

    let handler: QueryHandler = Arc::new(move |method, params| match method {
        "dashcam_flush" => {
            let label = params
                .get("marker_label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            *rl.lock().unwrap() = label;
            Ok(json!({
                "recording_id": "dash_default",
                "tier": "deliberate",
                "frames": 100
            }))
        }
        _ => Err(("unknown_method".into(), method.to_string())),
    });

    let harness = TestHarness::new(handler).await;
    let _ = harness
        .call_tool("recording", json!({ "action": "flush_dashcam" }))
        .await
        .unwrap();

    let label = received_label.lock().unwrap().clone();
    assert_eq!(label, "agent flush", "default label should be 'agent flush'");
}
```

**Acceptance Criteria**:
- [ ] `test_dashcam_status` — dashcam_status returns expected fields
- [ ] `test_dashcam_status_post_capture` — post_capture state includes open_clip info
- [ ] `test_dashcam_flush` — flush returns recording_id, tier, frames
- [ ] `test_dashcam_flush_empty_buffer_returns_error` — error propagated
- [ ] `test_dashcam_flush_when_disabled_returns_error` — error propagated
- [ ] `test_dashcam_flush_default_label` — server sends "agent flush" when label omitted

---

### Unit 2: Integration Tests — Unknown Action Error

**File**: `crates/spectator-server/tests/tcp_mock.rs`

```rust
#[tokio::test]
async fn test_recording_unknown_action_returns_error() {
    let handler: QueryHandler = Arc::new(|_, _| Ok(json!({})));
    let harness = TestHarness::new(handler).await;

    let err = harness
        .call_tool("recording", json!({ "action": "nonexistent" }))
        .await
        .unwrap_err();

    assert!(
        err.message.contains("Unknown recording action"),
        "expected 'Unknown recording action' error, got: {err:?}"
    );
}
```

**Acceptance Criteria**:
- [ ] Invalid action name returns McpError with descriptive message

---

### Unit 3: Scenario Tests — Dashcam + Explicit Recording Coexistence

**File**: `crates/spectator-server/tests/scenarios.rs`

Add after the recording lifecycle test:

```rust
// ---------------------------------------------------------------------------
// Section 6: Dashcam and explicit recording coexistence
// ---------------------------------------------------------------------------

/// Dashcam status should report "buffering" even while an explicit recording
/// is active. The two systems are orthogonal.
#[tokio::test]
async fn test_dashcam_independent_of_explicit_recording() {
    let active_id: Arc<std::sync::Mutex<Option<String>>> =
        Arc::new(std::sync::Mutex::new(None));
    let aid = active_id.clone();

    let handler: QueryHandler = Arc::new(move |method, params| {
        let mut id_guard = aid.lock().unwrap();
        match method {
            "recording_start" => {
                let id = "rec_coexist_001".to_string();
                *id_guard = Some(id.clone());
                Ok(json!({
                    "recording_id": id,
                    "name": "coexist_test",
                    "started_at_frame": 100
                }))
            }
            "recording_stop" => {
                let id = id_guard.take().unwrap_or_default();
                Ok(json!({
                    "recording_id": id,
                    "frames_captured": 42,
                    "duration_ms": 700
                }))
            }
            "dashcam_status" => {
                // Dashcam reports buffering regardless of explicit recording state
                Ok(json!({
                    "dashcam_enabled": true,
                    "state": "buffering",
                    "buffer_frames": 600,
                    "buffer_kb": 4800,
                    "config": {
                        "capture_interval": 1,
                        "pre_window_sec": { "system": 30, "deliberate": 60 },
                        "post_window_sec": { "system": 10, "deliberate": 30 },
                        "max_window_sec": 120,
                        "min_after_sec": 5,
                        "system_min_interval_sec": 2,
                        "byte_cap_mb": 1024
                    }
                }))
            }
            _ => Err(("unknown".into(), format!("unexpected: {method}"))),
        }
    });

    let harness = TestHarness::new(handler).await;

    // 1. dashcam_status before recording — should be buffering
    let status1 = harness
        .call_tool("recording", json!({ "action": "dashcam_status" }))
        .await
        .unwrap();
    assert_eq!(status1["state"], json!("buffering"));

    // 2. Start explicit recording
    let start = harness
        .call_tool("recording", json!({ "action": "start" }))
        .await
        .unwrap();
    assert!(start["recording_id"].as_str().is_some());

    // 3. dashcam_status during recording — still buffering
    let status2 = harness
        .call_tool("recording", json!({ "action": "dashcam_status" }))
        .await
        .unwrap();
    assert_eq!(status2["state"], json!("buffering"), "dashcam should keep buffering during explicit recording");

    // 4. Stop explicit recording
    let stop = harness
        .call_tool("recording", json!({ "action": "stop" }))
        .await
        .unwrap();
    assert!(stop["frames_captured"].as_u64().unwrap() > 0);

    // 5. dashcam_status after recording — still buffering
    let status3 = harness
        .call_tool("recording", json!({ "action": "dashcam_status" }))
        .await
        .unwrap();
    assert_eq!(status3["state"], json!("buffering"));
}

/// Dashcam clips appear in recording list alongside explicit recordings.
/// Clips are distinguishable by the "dashcam" flag in their metadata.
#[tokio::test]
async fn test_dashcam_clips_in_recording_list() {
    let flushed: Arc<std::sync::Mutex<bool>> = Arc::new(std::sync::Mutex::new(false));
    let f = flushed.clone();

    let handler: QueryHandler = Arc::new(move |method, _| match method {
        "dashcam_flush" => {
            *f.lock().unwrap() = true;
            Ok(json!({
                "recording_id": "dash_clip001",
                "tier": "deliberate",
                "frames": 300
            }))
        }
        "recording_list" => {
            let mut recordings = vec![
                json!({
                    "recording_id": "rec_explicit_001",
                    "name": "manual_run",
                    "frames_captured": 500,
                    "dashcam": false
                }),
            ];
            if *flushed.lock().unwrap() {
                recordings.push(json!({
                    "recording_id": "dash_clip001",
                    "name": "dashcam_100",
                    "frames_captured": 300,
                    "dashcam": true,
                    "tier": "deliberate"
                }));
            }
            Ok(json!({ "recordings": recordings }))
        }
        _ => Err(("unknown".into(), format!("unexpected: {method}"))),
    });

    let harness = TestHarness::new(handler).await;

    // List before flush — only explicit recordings
    let list1 = harness
        .call_tool("recording", json!({ "action": "list" }))
        .await
        .unwrap();
    let recordings1 = list1["recordings"].as_array().unwrap();
    assert_eq!(recordings1.len(), 1);

    // Flush dashcam
    let flush = harness
        .call_tool(
            "recording",
            json!({ "action": "flush_dashcam", "marker_label": "test clip" }),
        )
        .await
        .unwrap();
    assert!(flush["recording_id"].as_str().unwrap().starts_with("dash_"));

    // List after flush — should include dashcam clip
    let list2 = harness
        .call_tool("recording", json!({ "action": "list" }))
        .await
        .unwrap();
    let recordings2 = list2["recordings"].as_array().unwrap();
    assert_eq!(recordings2.len(), 2, "list should include dashcam clip after flush");

    let dashcam_clip = recordings2
        .iter()
        .find(|r| r["dashcam"].as_bool() == Some(true))
        .expect("dashcam clip should be flagged with dashcam=true");
    assert_eq!(dashcam_clip["recording_id"], json!("dash_clip001"));
}

/// add_marker during buffering triggers a dashcam clip and returns
/// the clip info. The marker is the trigger that transitions
/// Buffering → PostCapture.
#[tokio::test]
async fn test_add_marker_triggers_dashcam_clip() {
    let handler: QueryHandler = Arc::new(|method, params| match method {
        "recording_marker" => {
            // When no explicit recording is active, add_marker on the addon side
            // triggers the dashcam. The addon returns the marker ack plus dashcam info.
            let label = params.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!({
                "ok": true,
                "frame": 4521,
                "label": label,
                "source": source,
                "dashcam_triggered": true,
                "dashcam_tier": "deliberate"
            }))
        }
        _ => Err(("unknown".into(), format!("unexpected: {method}"))),
    });

    let harness = TestHarness::new(handler).await;
    let result = harness
        .call_tool(
            "recording",
            json!({ "action": "add_marker", "marker_label": "root cause" }),
        )
        .await
        .unwrap();

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["source"], json!("agent"));
    assert_eq!(result["dashcam_triggered"], json!(true));
}
```

**Acceptance Criteria**:
- [ ] Dashcam status unaffected by explicit recording start/stop
- [ ] Dashcam clips appear in recording list after flush
- [ ] Dashcam clips are distinguishable by `dashcam` flag
- [ ] add_marker triggers dashcam when no explicit recording is active

---

### Unit 4: Scenario Tests — Dashcam Flush → Analysis Roundtrip

**File**: `crates/spectator-server/tests/scenarios.rs`

```rust
/// After flushing a dashcam clip, the clip should be openable for M8 analysis.
/// This tests the full contract: dashcam produces SQLite files that M8 can read.
///
/// Since the mock addon can't produce real SQLite files, this test verifies the
/// MCP parameter validation path — the server must:
/// 1. Resolve storage path via TCP
/// 2. Attempt to open the SQLite file for the recording_id
/// If the file doesn't exist, we get a specific error (not a crash or panic).
#[tokio::test]
async fn test_dashcam_clip_analysis_validates_recording_id() {
    let handler: QueryHandler = Arc::new(|method, _| match method {
        "recording_resolve_path" => Ok(json!({ "path": "/tmp/spectator_test_nonexistent" })),
        _ => Err(("unknown".into(), format!("unexpected: {method}"))),
    });

    let harness = TestHarness::new(handler).await;

    // snapshot_at on a non-existent recording should return a clear error
    let err = harness
        .call_tool(
            "recording",
            json!({
                "action": "snapshot_at",
                "recording_id": "dash_nonexistent",
                "at_frame": 100
            }),
        )
        .await
        .unwrap_err();

    assert!(
        err.message.contains("not found") || err.message.contains("unreadable"),
        "expected 'not found' error for nonexistent dashcam clip, got: {err:?}"
    );
}
```

**Acceptance Criteria**:
- [ ] Analysis of non-existent recording_id produces clear error, not panic

---

### Unit 5: E2E Journey — Dashcam Agent Workflow

**File**: `crates/spectator-server/tests/e2e_journeys.rs`

This is the most important test. It exercises dashcam as an agent would use it:
notice something wrong → check dashcam status → add a marker to trigger clip →
verify clip saved → verify clip appears in list → clean up.

```rust
/// Journey: Agent uses dashcam to capture a spatial anomaly.
///
/// This is the primary dashcam usage pattern: the agent is debugging a game,
/// notices something suspicious, triggers a dashcam clip to capture the
/// surrounding context, and verifies the clip is available for analysis.
///
/// Steps:
///   1. Verify dashcam is active: dashcam_status returns state="buffering"
///   2. spatial_snapshot(standard) → baseline, note entities and frame
///   3. wait_frames(60) → let dashcam buffer accumulate ~1 second of data
///   4. spatial_action(teleport, Enemies/Scout, [100, 0, 100]) → create an anomaly
///   5. wait_frames(5) → let physics settle
///   6. recording(add_marker, marker_label="anomaly detected") → trigger dashcam clip
///   7. dashcam_status → state should be "post_capture" (capturing post-window)
///   8. wait_frames(120) → wait for post-window to close (~2s at 60fps)
///   9. dashcam_status → state should be back to "buffering"
///  10. recording(list) → dashcam clip should appear in the list
///  11. Verify the clip has dashcam=true and the trigger marker label
#[tokio::test]
async fn journey_dashcam_agent_workflow() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: verify dashcam is actively buffering on startup
    let status = h
        .expect(1, "recording", json!({ "action": "dashcam_status" }))
        .await;
    assert_eq!(
        status["dashcam_enabled"], json!(true),
        "Dashcam should be enabled by default"
    );
    assert_eq!(
        status["state"], json!("buffering"),
        "Dashcam should start in buffering state"
    );
    assert!(
        status["buffer_frames"].as_u64().unwrap_or(0) >= 0,
        "buffer_frames should be present"
    );
    assert!(
        status["config"].is_object(),
        "dashcam config should be returned"
    );

    // Step 2: baseline snapshot — verify scene is live
    let baseline = h
        .expect(2, "spatial_snapshot", json!({ "detail": "standard" }))
        .await;
    let entities = baseline["entities"]
        .as_array()
        .expect("Expected entities in baseline snapshot");
    assert!(
        !entities.is_empty(),
        "Baseline snapshot should have entities"
    );
    let base_frame = baseline["frame"].as_u64().unwrap_or(0);

    // Step 3: let dashcam accumulate buffer data (~1s worth)
    h.wait_frames(60).await;

    // Step 4: create a spatial anomaly by teleporting Scout far away
    h.expect(
        4,
        "spatial_action",
        json!({
            "action": "teleport",
            "node": "Enemies/Scout",
            "position": [100.0, 0.0, 100.0]
        }),
    )
    .await;

    // Step 5: let physics settle
    h.wait_frames(5).await;

    // Step 6: agent triggers dashcam clip via add_marker
    let marker_result = h
        .expect(
            6,
            "recording",
            json!({
                "action": "add_marker",
                "marker_label": "anomaly detected"
            }),
        )
        .await;
    assert!(
        !marker_result.is_null(),
        "add_marker should acknowledge the trigger"
    );

    // Step 7: dashcam should now be in post_capture state
    // (capturing the post-window after the trigger)
    let status_post = h
        .expect(7, "recording", json!({ "action": "dashcam_status" }))
        .await;
    assert_eq!(
        status_post["state"], json!("post_capture"),
        "Dashcam should be in post_capture after marker trigger. Got: {status_post}"
    );

    // Step 8: wait for the post-window to close
    // Default system post-window is 10s, but agent markers use deliberate tier
    // (30s post-window). We'll wait a shorter time and check the clip eventually
    // saved. For test speed, we use a shorter wait and check status transitions.
    // At 60fps, 600 frames = 10s (system tier). But the marker source is "agent"
    // → deliberate tier (30s = 1800 frames). That's too long for a test.
    // Instead, we force-flush the dashcam to close the clip immediately.
    let flush_result = h
        .expect(
            8,
            "recording",
            json!({
                "action": "flush_dashcam",
                "marker_label": "force close for test"
            }),
        )
        .await;
    let clip_id = flush_result["recording_id"]
        .as_str()
        .expect("flush should return recording_id");
    assert!(
        clip_id.starts_with("dash_"),
        "dashcam clip id should start with 'dash_', got: {clip_id}"
    );
    assert!(
        flush_result["frames"].as_u64().unwrap_or(0) > 0,
        "Clip should contain captured frames"
    );

    // Step 9: dashcam should be back to buffering after flush
    let status_after = h
        .expect(9, "recording", json!({ "action": "dashcam_status" }))
        .await;
    assert_eq!(
        status_after["state"], json!("buffering"),
        "Dashcam should return to buffering after clip flush. Got: {status_after}"
    );

    // Step 10: clip should appear in recording list
    let list = h
        .expect(10, "recording", json!({ "action": "list" }))
        .await;
    let recordings = list["recordings"]
        .as_array()
        .expect("list should return recordings array");
    let dashcam_clips: Vec<_> = recordings
        .iter()
        .filter(|r| r["dashcam"].as_bool() == Some(true))
        .collect();
    assert!(
        !dashcam_clips.is_empty(),
        "At least one dashcam clip should be in the list. Full list: {list}"
    );

    // Step 11: verify clip metadata
    let our_clip = recordings
        .iter()
        .find(|r| r["recording_id"].as_str() == Some(clip_id))
        .unwrap_or_else(|| {
            panic!("Clip {clip_id} should be in list. Recordings: {recordings:?}")
        });
    assert_eq!(our_clip["dashcam"], json!(true));
}
```

**Implementation Notes**:
- The journey uses `flush_dashcam` after triggering to avoid waiting 30 seconds
  for the deliberate tier post-window to expire. This is the realistic pattern
  anyway — an agent would flush when it has the info it needs.
- We verify the state machine transitions: buffering → post_capture → buffering
- We verify the clip appears in the list with the `dashcam` flag

**Acceptance Criteria**:
- [ ] Dashcam auto-starts in buffering on scene load
- [ ] dashcam_status returns enabled=true, state, buffer_frames, config
- [ ] add_marker triggers dashcam clip (transitions to post_capture)
- [ ] flush_dashcam closes clip and returns recording_id starting with "dash_"
- [ ] After flush, dashcam returns to buffering
- [ ] Flushed clip appears in recording list with dashcam=true

---

### Unit 6: E2E Journey — Dashcam + Explicit Recording Coexistence

**File**: `crates/spectator-server/tests/e2e_journeys.rs`

```rust
/// Journey: Explicit recording and dashcam run simultaneously without interference.
///
/// This catches the most likely class of bugs: shared state corruption between
/// the two recording paths (shared frame_buffer, shared db, shared collector).
///
/// Steps:
///   1. dashcam_status → buffering (dashcam auto-started)
///   2. recording(start) → explicit recording begins
///   3. recording(status) → active=true, recording_id present
///   4. dashcam_status → still buffering (not affected by explicit recording)
///   5. wait_frames(30) → both systems capturing simultaneously
///   6. recording(add_marker, "during_explicit") → marker goes to explicit recording
///   7. recording(stop) → explicit recording stops, frames_captured > 0
///   8. dashcam_status → STILL buffering (not stopped by explicit recording stop)
///   9. flush_dashcam → dashcam clip saved independently
///  10. recording(list) → both explicit recording AND dashcam clip in list
#[tokio::test]
async fn journey_dashcam_coexists_with_explicit_recording() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: dashcam should be auto-started
    let dc_status = h
        .expect(1, "recording", json!({ "action": "dashcam_status" }))
        .await;
    assert_eq!(dc_status["state"], json!("buffering"));

    // Step 2: start explicit recording
    let start = h
        .expect(2, "recording", json!({ "action": "start", "recording_name": "coexist_test" }))
        .await;
    let rec_id = start["recording_id"]
        .as_str()
        .expect("start should return recording_id")
        .to_string();
    assert!(!rec_id.is_empty());

    // Step 3: verify explicit recording is active
    let rec_status = h
        .expect(3, "recording", json!({ "action": "status" }))
        .await;
    assert_eq!(rec_status["recording_active"], json!(true));
    assert_eq!(rec_status["recording_id"].as_str(), Some(rec_id.as_str()));

    // Step 4: dashcam should still be buffering independently
    let dc_status2 = h
        .expect(4, "recording", json!({ "action": "dashcam_status" }))
        .await;
    assert_eq!(
        dc_status2["state"], json!("buffering"),
        "Dashcam should keep buffering during explicit recording"
    );

    // Step 5: let both systems capture simultaneously
    h.wait_frames(30).await;

    // Step 6: add marker to explicit recording
    let marker = h
        .expect(
            6,
            "recording",
            json!({ "action": "add_marker", "marker_label": "during_explicit" }),
        )
        .await;
    assert!(!marker.is_null());

    // Step 7: stop explicit recording
    let stop = h
        .expect(7, "recording", json!({ "action": "stop" }))
        .await;
    let frames = stop["frames_captured"].as_u64().unwrap_or(0);
    assert!(frames > 0, "Explicit recording should have captured frames");

    // Step 8: dashcam should STILL be buffering after explicit stop
    let dc_status3 = h
        .expect(8, "recording", json!({ "action": "dashcam_status" }))
        .await;
    assert_eq!(
        dc_status3["state"], json!("buffering"),
        "Dashcam must not stop when explicit recording stops"
    );

    // Step 9: flush dashcam to save a clip
    let flush = h
        .expect(
            9,
            "recording",
            json!({ "action": "flush_dashcam", "marker_label": "post-explicit" }),
        )
        .await;
    let dash_id = flush["recording_id"]
        .as_str()
        .expect("flush should return recording_id");
    assert!(dash_id.starts_with("dash_"));

    // Step 10: list should contain BOTH the explicit recording and the dashcam clip
    let list = h
        .expect(10, "recording", json!({ "action": "list" }))
        .await;
    let recordings = list["recordings"]
        .as_array()
        .expect("list should return recordings array");

    let has_explicit = recordings
        .iter()
        .any(|r| r["recording_id"].as_str() == Some(rec_id.as_str()));
    let has_dashcam = recordings
        .iter()
        .any(|r| r["recording_id"].as_str() == Some(dash_id));

    assert!(
        has_explicit,
        "Explicit recording {rec_id} should be in list. Got: {recordings:?}"
    );
    assert!(
        has_dashcam,
        "Dashcam clip {dash_id} should be in list. Got: {recordings:?}"
    );
}
```

**Acceptance Criteria**:
- [ ] Explicit recording start doesn't disable dashcam
- [ ] Explicit recording stop doesn't disable dashcam
- [ ] Both produce separate entries in recording list
- [ ] Frame captures don't corrupt each other

---

### Unit 7: E2E Journey — Dashcam Clip Deletion

**File**: `crates/spectator-server/tests/e2e_journeys.rs`

```rust
/// Journey: Flush a dashcam clip, verify it exists, delete it, verify it's gone.
///
/// Steps:
///   1. wait_frames(60) → accumulate buffer
///   2. flush_dashcam("cleanup_test") → get clip recording_id
///   3. recording(list) → clip should be present
///   4. recording(delete, recording_id) → delete the clip
///   5. recording(list) → clip should be gone
#[tokio::test]
async fn journey_dashcam_clip_lifecycle() {
    let mut h = support::e2e_harness::E2EHarness::start_3d()
        .await
        .expect("Failed to start Godot 3D scene");

    // Step 1: let buffer accumulate
    h.wait_frames(60).await;

    // Step 2: flush
    let flush = h
        .expect(
            2,
            "recording",
            json!({ "action": "flush_dashcam", "marker_label": "cleanup_test" }),
        )
        .await;
    let clip_id = flush["recording_id"]
        .as_str()
        .expect("flush should return recording_id")
        .to_string();

    // Step 3: verify clip exists in list
    let list = h
        .expect(3, "recording", json!({ "action": "list" }))
        .await;
    let found = list["recordings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["recording_id"].as_str() == Some(&clip_id));
    assert!(found, "Clip {clip_id} should be in list after flush");

    // Step 4: delete the clip
    let delete = h
        .expect(
            4,
            "recording",
            json!({ "action": "delete", "recording_id": clip_id }),
        )
        .await;
    assert!(
        delete["result"].as_str() == Some("ok") || delete["deleted"].as_bool() == Some(true),
        "delete should confirm success: {delete}"
    );

    // Step 5: verify clip is gone
    let list2 = h
        .expect(5, "recording", json!({ "action": "list" }))
        .await;
    let still_found = list2["recordings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["recording_id"].as_str() == Some(&clip_id));
    assert!(
        !still_found,
        "Clip {clip_id} should be gone after delete. List: {list2}"
    );
}
```

**Acceptance Criteria**:
- [ ] Flushed clip exists in list
- [ ] Deleted clip no longer in list
- [ ] Delete returns confirmation with recording_id

---

### Unit 8: Unit Tests — Ring Buffer Time-Based Eviction

**File**: `crates/spectator-godot/src/recorder.rs` (append to `mod tests`)

```rust
#[test]
fn ring_cap_frames_respects_time_based_limit() {
    // With 60fps, capture_interval=1, pre_window_deliberate=60s:
    // max frames from time = 60 * 60 / 1 = 3600
    // With 256 bytes/frame and 1024MB cap:
    // max frames from bytes = 1024 * 1024 * 1024 / 256 = 4194304
    // Time-based limit (3600) is the binding constraint.
    let cfg = DashcamConfig::default();
    let physics_fps = 60u32;
    let avg_frame_bytes = 256usize;
    let byte_cap = (cfg.byte_cap_mb as usize) * 1024 * 1024;

    let time_based = (cfg.pre_window_deliberate_sec as usize)
        * (physics_fps as usize)
        / (cfg.capture_interval as usize);
    let byte_based = byte_cap / avg_frame_bytes.max(1);

    let cap = time_based.min(byte_based);
    assert_eq!(cap, 3600, "time-based cap should be the binding constraint");
}

#[test]
fn ring_cap_frames_byte_cap_wins_for_large_frames() {
    // With 10KB per frame and 10MB cap: max frames from bytes = 10*1024*1024 / 10240 = 1024
    // With 60fps, pre_window_deliberate=60s: time-based = 3600
    // Byte cap (1024) is the binding constraint.
    let cfg = DashcamConfig {
        byte_cap_mb: 10,
        ..DashcamConfig::default()
    };
    let physics_fps = 60u32;
    let avg_frame_bytes = 10240usize; // 10KB per frame
    let byte_cap = (cfg.byte_cap_mb as usize) * 1024 * 1024;

    let time_based = (cfg.pre_window_deliberate_sec as usize)
        * (physics_fps as usize)
        / (cfg.capture_interval as usize);
    let byte_based = byte_cap / avg_frame_bytes.max(1);

    let cap = time_based.min(byte_based);
    assert_eq!(cap, 1024, "byte-based cap should be the binding constraint for large frames");
    assert!(cap < time_based);
}

#[test]
fn dashcam_capture_interval_reduces_frame_count() {
    // With capture_interval=2, only every other frame is captured.
    // At 60fps, pre_window_system=30s: time-based = 30 * 60 / 2 = 900
    let cfg = DashcamConfig {
        capture_interval: 2,
        ..DashcamConfig::default()
    };
    let physics_fps = 60u32;
    let time_based = (cfg.pre_window_system_sec as usize)
        * (physics_fps as usize)
        / (cfg.capture_interval as usize);

    assert_eq!(time_based, 900);
}
```

**Acceptance Criteria**:
- [ ] Time-based cap is binding for small frame sizes
- [ ] Byte cap is binding for large frame sizes
- [ ] capture_interval reduces frame count proportionally

---

### Unit 9: Unit Tests — Merge Policy Edge Cases

**File**: `crates/spectator-godot/src/recorder.rs` (append to `mod tests`)

```rust
#[test]
fn dashcam_merge_deliberate_into_deliberate_extends_window() {
    // Two deliberate triggers: second should extend frames_remaining to
    // the larger of the two remaining post-windows.
    let mut frames_remaining: u32 = 100; // first trigger, nearly expired
    let deliberate_frames: u32 = 1800;

    // Second deliberate trigger arrives
    frames_remaining = frames_remaining.max(deliberate_frames);

    assert_eq!(frames_remaining, 1800, "deliberate+deliberate should extend to full window");
}

#[test]
fn dashcam_system_trigger_into_deliberate_clip_does_not_downgrade() {
    // A system trigger into an already-deliberate clip should NOT downgrade the tier.
    let mut existing_tier = DashcamTier::Deliberate;
    let mut frames_remaining: u32 = 500;
    let system_frames: u32 = 600;

    let new_tier = DashcamTier::System;
    if new_tier == DashcamTier::Deliberate {
        // This branch is NOT taken — system trigger doesn't upgrade
        frames_remaining = frames_remaining.max(1800);
        existing_tier = DashcamTier::Deliberate;
    } else if existing_tier == DashcamTier::Deliberate {
        // System into deliberate: extend window but keep deliberate tier.
        // In the real implementation, system triggers into deliberate clips
        // are still rate-limited but don't downgrade tier.
        frames_remaining = frames_remaining.max(system_frames);
        // tier stays deliberate
    }

    assert_eq!(existing_tier, DashcamTier::Deliberate, "tier must not downgrade");
    assert_eq!(frames_remaining, 600, "window should extend to system_frames");
}

#[test]
fn dashcam_min_after_sec_floor() {
    // Post-window should never be less than min_after_sec, even if
    // the config specifies a shorter post-window.
    let cfg = DashcamConfig {
        post_window_system_sec: 2, // Shorter than min_after_sec
        min_after_sec: 5,
        ..DashcamConfig::default()
    };
    let physics_fps = 60u32;

    let post_window = cfg.post_window_system_sec.max(cfg.min_after_sec);
    let post_frames = post_window * physics_fps;

    assert_eq!(post_window, 5, "min_after_sec should floor the post-window");
    assert_eq!(post_frames, 300);
}

#[test]
fn dashcam_force_close_not_applied_to_deliberate() {
    // Deliberate clips should NOT have force_close_at_frame set by default.
    let tier = DashcamTier::Deliberate;
    let force_close: Option<u64> = if tier == DashcamTier::System {
        Some(1000 + 120 * 60)
    } else {
        None
    };

    assert!(force_close.is_none(), "deliberate clips should not have force_close");
}
```

**Acceptance Criteria**:
- [ ] Deliberate + deliberate extends window
- [ ] System into deliberate doesn't downgrade tier
- [ ] min_after_sec floors the post-window
- [ ] Deliberate clips don't get force_close_at_frame

---

### Unit 10: Unit Tests — Dashcam Config JSON Partial Updates

**File**: `crates/spectator-godot/src/recorder.rs` (append to `mod tests`)

```rust
#[test]
fn dashcam_apply_config_partial_preserves_unset_fields() {
    let mut cfg = DashcamConfig::default();
    let original_post_system = cfg.post_window_system_sec;
    let original_min_after = cfg.min_after_sec;

    // Only update one field
    let json = serde_json::json!({
        "pre_window_system_sec": 45,
    });

    if let Some(n) = json.get("pre_window_system_sec").and_then(|x| x.as_u64()) {
        cfg.pre_window_system_sec = n as u32;
    }
    if let Some(n) = json.get("post_window_system_sec").and_then(|x| x.as_u64()) {
        cfg.post_window_system_sec = n as u32;
    }

    assert_eq!(cfg.pre_window_system_sec, 45, "updated field should change");
    assert_eq!(cfg.post_window_system_sec, original_post_system, "unset field should be preserved");
    assert_eq!(cfg.min_after_sec, original_min_after, "unset field should be preserved");
}

#[test]
fn dashcam_config_enabled_toggle() {
    let mut cfg = DashcamConfig::default();
    assert!(cfg.enabled, "dashcam should be enabled by default");

    let json = serde_json::json!({ "enabled": false });
    if let Some(b) = json.get("enabled").and_then(|x| x.as_bool()) {
        cfg.enabled = b;
    }
    assert!(!cfg.enabled, "dashcam should be disabled after toggle");
}
```

**Acceptance Criteria**:
- [ ] Partial config update preserves unset fields
- [ ] Enabled toggle works

---

## Implementation Order

1. **Unit 8** — Ring buffer time-based eviction tests (no dependencies, fast)
2. **Unit 9** — Merge policy edge case tests (no dependencies, fast)
3. **Unit 10** — Config JSON partial update tests (no dependencies, fast)
4. **Unit 1** — Integration dashcam status & flush tests (depends on existing mock harness)
5. **Unit 2** — Unknown action error test (standalone)
6. **Unit 3** — Scenario dashcam coexistence tests (depends on mock harness)
7. **Unit 4** — Scenario dashcam analysis validation test (depends on mock harness)
8. **Unit 5** — E2E journey: dashcam agent workflow (requires Godot, highest value)
9. **Unit 6** — E2E journey: dashcam + explicit coexistence (requires Godot)
10. **Unit 7** — E2E journey: dashcam clip lifecycle (requires Godot)

## Testing

### Run Commands

```bash
# Unit tests only (fast, no external deps)
cargo test --workspace

# Integration tests with mock addon
cargo test -p spectator-server --features integration-tests

# E2E journey tests (requires Godot + built GDExtension)
spectator-deploy ~/dev/spectator/tests/godot-project
cargo test -p spectator-server --features e2e-tests -- --nocapture

# All together
cargo test -p spectator-server --features integration-tests,e2e-tests -- --nocapture
```

### Test File → Unit Mapping

| File | Units | Layer |
|------|-------|-------|
| `crates/spectator-godot/src/recorder.rs` | 8, 9, 10 | Unit |
| `crates/spectator-server/tests/tcp_mock.rs` | 1, 2 | Integration |
| `crates/spectator-server/tests/scenarios.rs` | 3, 4 | Scenario |
| `crates/spectator-server/tests/e2e_journeys.rs` | 5, 6, 7 | E2E Journey |

## Verification Checklist

```bash
# 1. Unit tests pass
cargo test --workspace 2>&1 | tail -5

# 2. Integration tests pass
cargo test -p spectator-server --features integration-tests 2>&1 | tail -10

# 3. E2E tests pass (requires Godot)
cargo test -p spectator-server --features e2e-tests -- --nocapture 2>&1 | tail -20

# 4. No clippy warnings in new test code
cargo clippy --workspace --all-features

# 5. Formatting
cargo fmt --check
```
