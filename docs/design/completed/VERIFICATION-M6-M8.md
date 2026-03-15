# Verification Report: Milestones 6, 7, 8

**Target:** M6 (Editor Dock), M7 (Recording Capture), M8 (Recording Analysis)
**Design docs:** `docs/design/M6-EDITOR-DOCK.md`, `docs/design/M7-RECORDING-CAPTURE.md`, `docs/design/M8-RECORDING-ANALYSIS.md`
**Foundation docs checked:** `docs/ROADMAP.md`, `CLAUDE.md`, `.claude/rules/patterns.md`, `.claude/skills/patterns/*.md`

---

## Status: FAIL

---

## Build & Tests

- **Build:** PASS (5 warnings — all intentional forward-looking fields, documented below)
- **Tests:** PASS (50 passed, 0 failed)

### Build warnings (not compliance gaps)

These warnings are intentional: forward-use fields for future expansion, consistent with code comments.

| Warning | File | Reason |
|---|---|---|
| `read_recording_meta` never used | `recording_analysis.rs:107` | Utility for future features |
| `signal` field never read | `recording_analysis.rs:254` | Future `signal_emitted` condition |
| `HandshakeInfo` fields | `tcp.rs` | 2D/3D dimension detection (M9) |
| `marker_frame` field | `recording.rs:44` | Future frame-specific markers |
| `include_signals`, `include_input` | `recording.rs:93-94` | Future capture filtering |

---

## Design Compliance

### Milestone 6: Editor Dock

- [x] Unit 1: Connection status queries (`get_connection_status`, `get_port`, `activity_received` signal) — complete
- [x] Unit 2: Activity log protocol (signal-based, server → addon push via `Message::Event`) — complete
- [x] Unit 3: Dock panel scene (`dock.tscn`, VBoxContainer layout, all unique-named nodes) — complete
- [x] Unit 4: Dock panel script (`dock.gd`, 1 Hz updates, status colors, activity feed, collapse) — complete
- [x] Unit 5: Plugin integration (dock added in `_enter_tree`, runtime static `instance` var pattern) — complete
- [x] Unit 6: In-game overlays (CanvasLayer layer=128, pause label, toast container, recording dot, F8/F9/F10) — complete
- [x] Unit 7: Server-side activity logging (`activity.rs`, all summary functions, `log_activity` helper) — complete
- [ ] **GAP M6-1:** Watch count display uses fragile string-parsing (Option 1) instead of `meta.active_watches` field (Option 2)
- [ ] **GAP M6-2 (minor):** StatusDot uses `ColorRect` instead of `TextureRect` as specified in design
- [ ] **GAP M6-3 (minor):** Watch and recording activity entries use BBCode color "cyan" instead of "blue" as specified in design table

### Milestone 7: Recording Capture

- [x] Unit 1: Workspace dependencies (`rusqlite` bundled, `rmp-serde`) — complete
- [x] Unit 2: `StageRecorder` GDExtension class (frame capture in `_physics_process`, SQLite+WAL, MessagePack, signals, flush every 60 frames) — complete
- [x] Unit 3: Recording query handler (`recording_handler.rs`, all 7 capture actions dispatched via TCP) — complete
- [x] Unit 4: `recording` MCP tool (start, stop, status, list, delete, markers, add_marker) — complete
- [x] Unit 5: Activity logging for recording summaries — complete
- [x] Unit 6: Dock recording section (RecordBtn, StopBtn, MarkerBtn, RecordingStats, RecordingLibrary in dock.tscn + dock.gd) — complete
- [x] Unit 7: Keyboard shortcuts and recording indicator (F8 toggle, F9 marker+yellow flash, F10 pause, red dot) — complete
- [x] Unit 8: GDExtension registration (`StageRecorder` in lib.rs) — complete

### Milestone 8: Recording Analysis

- [x] Unit 1: `FrameEntityData` in `stage-protocol` (shared between godot + server crates) — complete
- [x] Unit 2: Storage path resolution (`recording_resolve_path` TCP method, `resolve_storage_path()` cache) — complete
- [x] Unit 3a: Frame deserialization (`read_frame`, `read_frame_at_time`) — complete
- [x] Unit 3b: `snapshot_at` (frame/time targeting, same shape as `spatial_snapshot`, token budget) — complete
- [x] Unit 3c: `query_range` (proximity, velocity_spike, property_change, state_transition conditions; first_breach, deepest_penetration annotations; budget truncation) — complete
- [x] Unit 3d: `diff_frames` (position delta, state changes, markers between frames) — complete
- [x] Unit 3e: `find_event` (events + markers tables, type filter, node filter, frame range) — complete
- [x] Unit 4: Extended `RecordingParams` (at_frame, at_time_ms, from_frame, to_frame, frame_a, frame_b, condition, event_type, event_filter) — complete
- [x] Unit 5: Handler routing (11 actions dispatched in `handle_recording`) — complete
- [x] Unit 6: Activity logging for all 4 analysis actions — complete
- [x] Unit 7: MCP tool description lists all 11 actions — complete
- [x] Unit 8: Server dependencies (`rusqlite`, `rmp-serde` in stage-server Cargo.toml) — complete
- [x] Unit 9: `values_equal` exported from `stage_core::delta` — complete
- [x] Unit 10: System marker generation (velocity spike post-hoc in query_range) — complete
- [x] Unit 11: TCP dispatch for `recording_resolve_path` — complete

---

## Code Quality

- [x] Pattern: `mcp-tool-handler` — all 10 tools use `#[tool_router]`, `Parameters<T>`, `finalize_response`, `log_activity`
- [x] Pattern: `tcp-length-prefix` — 4-byte BE u32 + JSON; codec used in both crates; `MAX_MESSAGE_SIZE` enforced
- [x] Pattern: `arc-mutex-state` — `Arc<Mutex<SessionState>>` with oneshot channels; locks released before `await`
- [x] Pattern: `gdext-class` — `StageRecorder` uses `#[derive(GodotClass)]`, `INode` lifecycle, `#[func]`/`#[signal]`
- [x] Pattern: `serde-tagged-enum` — `#[serde(tag="type")]` with per-variant `#[serde(rename="...")]`
- [x] Pattern: `error-layering` — `CodecError` → `anyhow::Result` → `McpError`; no `.unwrap()` in library code
- [x] Pattern: `inline-test-fixtures` — 15 tests in `recording_analysis.rs`, builder functions (`test_entity()`, `test_db()`)
- [x] Logging: No `println!` in stage-server — all logging via `tracing` or `eprintln!`
- [x] Error handling: `McpError::internal_error` / `McpError::invalid_params` used consistently
- [x] SQL safety: All rusqlite queries parameterized; no string interpolation of user values
- [x] Path safety: Recording IDs extracted via `file_stem()`, not passed directly from user input to filesystem

---

## Gaps (Action Required)

### Gap 1 — [M6 design compliance] Watch count uses fragile summary-string parsing instead of `meta.active_watches`

- **File (server):** `crates/stage-server/src/activity.rs:13-28` — `build_activity_message` does not include `meta` field
- **File (dock):** `addons/stage/dock.gd:237-243` — derives watch count by checking `summary.begins_with("Watching ")`
- **Expected:** Design (M6-EDITOR-DOCK.md, line 1011-1023) explicitly chose Option 2: server includes `meta: { "active_watches": N }` in watch activity events. The design called Option 1 (summary string parsing) "fragile" and rejected it.
- **Actual:** `build_activity_message` sends only `entry_type`, `summary`, `tool`, `timestamp`. No `meta` field. Dock parses summary text to infer add/remove/clear operations and increments/decrements a local counter.
- **Fix (server):** In `build_activity_message`, add an optional `meta` parameter. In `watch_summary()` callers in `mcp/mod.rs`, pass the current watch count from `WatchEngine::watch_count()`. Build the message with `meta: { "active_watches": N }`. Alternatively, add a separate `build_watch_activity_message(summary, tool, active_watches)` function.
- **Fix (dock):** In `_on_activity_received`, when `entry_type == "watch"`, read `meta.active_watches` from the event data (via a new signal parameter or by reading it from the event dictionary) instead of parsing the summary string.

---

### Gap 2 (minor) — [M6 design compliance] StatusDot node uses `ColorRect` instead of `TextureRect`

- **File:** `addons/stage/dock.tscn:7` — `[node name="StatusDot" type="ColorRect" ...]`
- **File:** `addons/stage/dock.gd:16` — `@onready var status_dot: ColorRect = %StatusDot`
- **Expected:** M6-EDITOR-DOCK.md line 302 specifies `TextureRect ("StatusDot")  — 12x12, colored circle`
- **Actual:** `ColorRect` is used, giving a square indicator instead of a circular one
- **Fix:** Change `StatusDot` to `TextureRect` in dock.tscn and update dock.gd type annotation to `TextureRect`. Use a circular icon texture, or accept the square as a conscious deviation (update the design doc to reflect it).

---

### Gap 3 (minor) — [M6 design compliance] Watch/recording activity entries use "cyan" instead of "blue"

- **File:** `addons/stage/dock.gd:267-271` — `"watch": color = "cyan"`, `"recording": color = "cyan"`
- **Expected:** M6-EDITOR-DOCK.md line 126 table specifies `watch → Blue`, `recording → Blue`
- **Actual:** BBCode color `"cyan"` is used for both watch and recording entries
- **Fix:** Change `"cyan"` to `"blue"` for "watch" and "recording" entries in `_add_activity_entry`, or update the design doc to reflect that cyan was chosen for legibility.

---

## Summary

| Milestone | Status | Gaps |
|---|---|---|
| M6: Editor Dock | FAIL | 3 gaps (1 functional, 2 minor) |
| M7: Recording Capture | PASS | 0 gaps |
| M8: Recording Analysis | PASS | 0 gaps |

M7 and M8 are fully implemented and match their design docs. M6 has one functional gap (watch count `meta` field) and two minor cosmetic deviations (node type, color name). The functional gap means the dock's "Watches: N active" display is based on fragile string matching rather than the authoritative server-side count the design specified.
