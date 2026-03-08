# Design: Milestone 11 — Dashcam Recording

## Overview

M11 delivers always-on dashcam recording: the addon maintains a rolling in-memory ring
buffer from the moment Godot loads the extension. When a marker fires — from game code,
an agent, or a human — the system saves a clip: the pre-window frames already in the
buffer plus a post-window of new frames captured after the trigger. Each clip is a
self-contained SQLite file, identical in schema to M7 explicit recordings.

**Depends on:** M7 (SpectatorRecorder, SQLite schema, FrameEntityData), M8 (analysis
queries work unchanged on dashcam clips)

**Exit Criteria:**
- Addon starts buffering automatically with no MCP or human interaction.
- Game code calls `SpectatorRecorder.add_marker("system", "player_died")` from GDScript;
  a clip is saved to `user://spectator_recordings/` covering the configured pre- and
  post-window around that frame.
- Agent calls `recording(action: "add_marker", marker_label: "root cause")` with
  `source: "agent"`; clip saves with agent-tier windows.
- Human presses F9 in the dock; clip saves with human-tier windows.
- Overlapping triggers from the same incident produce one merged clip, not multiple.
- `recording(action: "list")` returns dashcam clips alongside explicit recordings.
- All M8 analysis actions (`snapshot_at`, `query_range`, `diff_frames`, `find_event`)
  work on dashcam clips unchanged.

---

## Architecture Decision: Ring Buffer in SpectatorRecorder

**Decision:** Add a second operating mode to `SpectatorRecorder` — dashcam mode —
that runs a fixed-capacity `VecDeque<CapturedFrame>` alongside the existing explicit
recording path. Dashcam mode activates automatically in `fn ready()`.

**Rationale:**
- The existing `_physics_process` capture loop and `CapturedFrame` type are reused
  with no protocol changes.
- A `VecDeque` with a configured frame cap provides O(1) eviction of the oldest frame.
- Separating dashcam state from explicit recording state keeps the two modes
  orthogonal — a developer can still do an explicit `recording(action: "start")` while
  dashcam runs in the background (they produce separate files).
- No new TCP methods are needed. Clips appear in `recording_list` automatically because
  they use the same SQLite schema and storage directory.

**Consequence:** `SpectatorRecorder` grows a dashcam config struct and clip-state
machine. The `recorder.rs` file will be the primary site of change; no other crates
require new logic.

---

## Architecture Decision: Per-Tier Window Configuration

**Decision:** Dashcam pre/post windows are configured per marker tier (`system`,
`agent`/`human`), not globally. Agent and human share one config since both represent
deliberate, high-signal triggers.

**Rationale:**
- System markers are typically frequent and mechanical (death events, level transitions).
  A shorter window keeps clips focused and reduces SQLite file count.
- Human and agent markers are deliberate — the developer or agent noticed something.
  A longer window provides full context.
- Sharing one config for agent+human reduces the config surface without meaningful loss
  of control.

**Defaults:**

| Tier | Pre-window | Post-window |
|---|---|---|
| `system` | 30 s | 10 s |
| `agent` / `human` | 60 s | 30 s |

All values are configurable via `spatial_config` (session) and `spectator.toml`
(project), consistent with M5 config precedence.

---

## Architecture Decision: Clip State Machine

**Decision:** After a marker fires, the recorder enters a `PostCapture` state that
tracks the remaining post-window duration. The ring buffer continues filling normally.
When the post-window elapses, the clip is flushed to SQLite and the recorder returns to
`Buffering` state.

```
Buffering ──trigger──▶ PostCapture(frames_remaining, clip_meta)
PostCapture ──frame──▶ PostCapture (frames_remaining - 1)
PostCapture ──0──────▶ flush_clip() ──▶ Buffering
```

**Why not flush immediately on trigger?**
Flushing immediately would save the pre-window but miss the post-window. The trigger
frame is the interesting moment; what happens immediately after is often the most
diagnostic data (e.g. the enemy that killed the player, the physics body that clipped
through the floor).

**Consequence:** During `PostCapture`, incoming frames are written to both the ring
buffer (pre-window for any subsequent trigger) and a separate `post_buffer` that will
become the tail of the clip. This allows overlapping triggers to extend the post-window
or upgrade the clip tier without re-reading the ring buffer.

---

## Architecture Decision: Merge Policy

**Decision:** While in `PostCapture`, a new trigger extends or upgrades the open clip
rather than creating a second file.

Rules:
1. **System + system within open clip:** extend `frames_remaining` to
   `max(frames_remaining, post_window_sec * physics_fps)`. Clip stays system-tier.
2. **Agent/human into open system clip:** upgrade clip tier to agent/human, extend
   `frames_remaining` to agent/human post-window. The existing system markers in the
   clip are preserved as annotations.
3. **Agent/human + agent/human:** extend `frames_remaining` to the larger of the two
   post-windows. Tier stays agent/human.
4. **Any trigger after clip closes:** new clip, independent file.

**Max window guard:** A system-tier clip that has been in `PostCapture` longer than
`max_window_sec` (default: 120 s) is force-closed, even if new system markers keep
extending it. This prevents a noisy system trigger from accumulating an unbounded clip.
The `min_after_sec` (default: 5 s) ensures the post-window never closes so quickly that
frames are clipped mid-incident.

Human/agent clips are not subject to `max_window_sec` by default — deliberate triggers
are assumed intentional regardless of duration. This is configurable.

---

## Architecture Decision: System Marker Rate Limiting

**Decision:** Consecutive system markers that fire within `system_min_interval_sec`
(default: 2 s) while a clip is already open are recorded as marker annotations in the
existing clip but do not extend `frames_remaining`. Rate limiting only applies to
system-tier; agent and human markers always extend/upgrade.

**Rationale:** Game code can emit `add_marker("system", ...)` from a damage handler
that fires hundreds of times per second. Without rate limiting, each call would extend
the post-window, potentially holding `PostCapture` open indefinitely. The annotations
are still written to the clip so the frequency of events is visible to the agent.

---

## New GDExtension API

### SpectatorRecorder — new `@func` exports

```gdscript
# Enable/disable dashcam mode at runtime (enabled by default on load).
func set_dashcam_enabled(enabled: bool) -> void

# Returns true if dashcam is actively buffering or in post-capture.
func is_dashcam_active() -> bool

# Returns the current ring buffer size in frames.
func get_dashcam_buffer_frames() -> int

# Returns the current ring buffer memory usage in KB.
func get_dashcam_buffer_kb() -> int

# Returns the dashcam clip state: "buffering", "post_capture", or "disabled".
func get_dashcam_state() -> String

# Force-flush the current ring buffer to a clip immediately, regardless of state.
# Useful for "save the last N seconds right now" from game code or a keyboard shortcut.
func flush_dashcam_clip(label: String) -> String  # returns clip recording_id or ""
```

The existing `add_marker(source, label)` func is the primary trigger. No new API is
needed for triggering — game code already calls this.

### New signals

```gdscript
signal dashcam_clip_saved(recording_id: String, tier: String, frames: int)
signal dashcam_clip_started(trigger_frame: int, tier: String)
```

---

## New MCP Tool Parameters

The `recording` tool gains two new actions:

### `action: "dashcam_status"`

Returns dashcam state, ring buffer stats, and open clip info if in post-capture.

```json
{
  "action": "dashcam_status"
}
```

Response:
```json
{
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
}
```

### `action: "flush_dashcam"`

Force-saves whatever is in the ring buffer right now, labelled as an agent clip.

```json
{
  "action": "flush_dashcam",
  "marker_label": "suspected physics glitch"
}
```

---

## New TCP Methods (addon ↔ server)

| Method | Direction | Purpose |
|---|---|---|
| `dashcam_status` | server → addon | Returns dashcam state and config |
| `dashcam_flush` | server → addon | Force-flushes ring buffer to clip |
| `dashcam_config` | server → addon | Updates dashcam config at runtime |

These follow the existing `query_addon` / `handle_recording_query` dispatch pattern.

---

## Configuration

New keys under `[recording]` in `spectator.toml`:

```toml
[recording]
dashcam_enabled = true
dashcam_capture_interval = 1          # physics frames between captures
dashcam_pre_window_system_sec = 30
dashcam_pre_window_deliberate_sec = 60
dashcam_post_window_system_sec = 10
dashcam_post_window_deliberate_sec = 30
dashcam_max_window_sec = 120          # force-close system clips after this long
dashcam_min_after_sec = 5             # post-window floor
dashcam_system_min_interval_sec = 2   # rate-limit on system marker triggers
dashcam_byte_cap_mb = 1024            # ring buffer memory cap
```

`spatial_config` session overrides follow the same precedence as M5.

---

## Memory Model

Ring buffer frame count is derived from config:

```
max_frames = min(
    pre_window_sec * physics_fps / capture_interval,
    byte_cap_mb * 1024 * 1024 / avg_frame_bytes
)
```

`avg_frame_bytes` is estimated from the first 60 frames captured and updated every
300 frames. If the byte cap is reached before the time-based frame count, the byte cap
wins and the effective pre-window shrinks. `dashcam_status` reports both the configured
pre-window and the actual achievable pre-window given current memory usage, so the agent
can inform the developer if the byte cap is constraining coverage.

Physics FPS is read from `Engine.physics_ticks_per_second` once on init and cached.

---

## SQLite Clip Metadata

Dashcam clips use the same schema as M7 explicit recordings. The `capture_config`
JSON column in the `recording` table gains two new fields to distinguish them:

```json
{
  "capture_interval": 1,
  "max_frames": 1800,
  "dashcam": true,
  "tier": "system",
  "triggers": [
    { "frame": 4521, "source": "system", "label": "player_died" },
    { "frame": 4580, "source": "human",  "label": "happened again" }
  ]
}
```

This allows `recording_list` to surface dashcam clips differently in the dock UI (M12
concern) while keeping the analysis path (M8) completely unchanged.

---

## Dock Integration

The dashcam state is surfaced in the existing recording dock panel:

- A persistent status line: `● Dashcam: buffering (14.4 MB, ~30 s)` or
  `◉ Dashcam: saving clip…`
- F9 in dashcam mode calls `flush_dashcam_clip("human")` instead of adding a marker to
  an explicit recording. If an explicit recording is also active, F9 adds a marker to
  it as before (explicit recording takes priority for F9).
- Saved dashcam clips appear in the recording library list tagged `[dashcam]`.

Dock changes are minimal — the existing recording panel is extended, not replaced.

---

## Implementation Plan

### Phase 1 — Ring buffer (GDExtension)
- Add `DashcamConfig` struct and `DashcamState` enum to `recorder.rs`
- Add `ring_buffer: VecDeque<CapturedFrame>` alongside existing `frame_buffer`
- Auto-start dashcam in `fn ready()` with defaults
- Evict oldest frame when byte cap exceeded
- Add new `@func` exports and signals

### Phase 2 — Clip state machine (GDExtension)
- Implement `PostCapture` state with `post_buffer`
- Wire `add_marker` to trigger state transition
- Implement merge/upgrade/rate-limit logic
- Implement `flush_clip()` — writes pre + post buffers to new SQLite file

### Phase 3 — TCP + MCP (server)
- Add `dashcam_status`, `dashcam_flush`, `dashcam_config` to `recording_handler.rs`
- Add `action: "dashcam_status"` and `action: "flush_dashcam"` to `recording.rs`
- Wire through `handle_recording_query` dispatch

### Phase 4 — Config (server + godot)
- Add dashcam keys to `spectator.toml` parsing in `config.rs`
- Add `spatial_config` session override support for dashcam params
- Propagate config to addon via `dashcam_config` TCP method on connect

### Phase 5 — Dock
- Add dashcam status line to recording dock panel in `runtime.gd`
- Update F9 shortcut to handle dashcam-only mode
- Tag dashcam clips in library list

### Phase 6 — Tests
- Ring buffer eviction at byte cap
- Merge logic: system+system, human into system, human+human
- Rate limiting: rapid system markers within interval
- Max window force-close
- Clip SQLite output: correct pre + post frames, correct markers table
- `dashcam_status` TCP response shape
