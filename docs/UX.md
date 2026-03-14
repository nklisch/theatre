# Stage — UX Design

## Interaction Surfaces

Stage has two users interacting through three surfaces:

| User | Surface | Interaction |
|---|---|---|
| AI Agent | MCP Tools (9 tools) | Structured queries, actions, configuration |
| Human Developer | Editor Dock Panel | Visual status, recording controls, activity feed |
| Human Developer | In-Game Keyboard Shortcuts | Recording toggle, markers, pause |

The human and agent share a **common workspace** (the running game) and a **common timeline** (recordings). The dock panel and MCP tools are windows into the same underlying state.

---

## Editor Dock Panel

The dock panel is added to Godot's editor by the EditorPlugin. It appears in the left or right sidebar (user-movable). It provides at-a-glance status and recording controls without requiring the human to interact with the AI client.

### Layout

```
┌─ Stage ─────────────────────────────────┐
│                                              │
│  ● Connected  |  Port 9077                   │  ← Connection status
│                                              │
│  ┌─ Recording ────────────────────────────┐  │
│  │  [● Record]  [■ Stop]  [⚑ Marker]     │  │  ← Recording controls
│  │                                         │  │
│  │  ⏱ 00:12.4  |  Frame 4580  |  1.2 MB  │  │  ← Recording stats
│  └─────────────────────────────────────────┘  │
│                                              │
│  ┌─ Session ──────────────────────────────┐  │
│  │  Tracking: 47 nodes (3 groups)         │  │  ← Active tracking info
│  │  Watches: 2 active                     │  │
│  │  Frame: 4580  |  60 fps                │  │
│  └─────────────────────────────────────────┘  │
│                                              │
│  ┌─ Recordings ───────────────────────────┐  │
│  │  wall_clip_repro    12.4s  Today  [×]  │  │  ← Recording library
│  │  patrol_test_03      8.1s  Today  [×]  │  │
│  │  physics_debug      22.0s  Mar 4  [×]  │  │
│  └─────────────────────────────────────────┘  │
│                                              │
│  ┌─ Agent Activity ───────────────────────┐  │
│  │  12:04:12  Inspecting scout_02         │  │  ← Agent activity feed
│  │  12:04:14  Watching group:enemies      │  │
│  │  12:04:18  Teleported scout_02         │  │
│  │            → [5.0, 0.0, -3.0]          │  │
│  │  12:04:19  Set collision_mask = 7      │  │
│  │            on scout_02                 │  │
│  └─────────────────────────────────────────┘  │
│                                              │
└──────────────────────────────────────────────┘
```

### Section Details

#### Connection Status

- **Green dot** + "Connected" when MCP server is connected via TCP
- **Red dot** + "Disconnected" when no connection
- **Yellow dot** + "Waiting..." when TCP server is listening but no client
- Port number always visible
- Updates in real-time (checked every frame via TCP server status)

#### Recording Controls

- **Record button** (red circle icon): Starts a new recording. If no name is provided, auto-generates one from timestamp ("recording_2026-03-05_14-30").
  - Opens a small dialog for optional name input
  - Disabled while a recording is already active
  - Keyboard shortcut: F8

- **Stop button** (square icon): Ends the active recording.
  - Disabled when not recording
  - Keyboard shortcut: F8 (toggle)

- **Marker button** (flag icon): Drops a marker at the current frame.
  - Opens a small text input for optional label
  - Quick-press drops a marker with no label
  - Disabled when not recording
  - Keyboard shortcut: F9

- **Stats line**: Visible during recording. Shows elapsed time, current frame count, and estimated memory usage of the recording buffer.

#### Session Info

Shows the current state of the Stage session:
- **Tracking**: Number of nodes being actively monitored + group count
- **Watches**: Number of active watch subscriptions (set by agent)
- **Frame**: Current physics frame number
- **FPS**: Current frames per second (from Engine)

This section updates every second (not every frame — to avoid UI overhead).

#### Recording Library

List of saved recordings, sorted by date (most recent first). Each entry shows:
- Name
- Duration
- Date (relative: "Today", "Yesterday", "Mar 4")
- Delete button (×) with confirmation dialog

Clicking a recording doesn't do anything directly — the agent accesses recordings via MCP tools. But the list gives the human visibility into what's available.

#### Agent Activity Feed

A scrolling log showing what the agent is doing. This is the primary trust-building mechanism — the human can see that the agent is working and what it's touching.

Entries come from the MCP server via TCP push messages (`activity_log` events). Types:

| Entry Type | Example | Style |
|---|---|---|
| Query | "Inspecting scout_02" | Neutral (gray text) |
| Query | "Snapshot (summary, radius: 50)" | Neutral |
| Watch | "Watching group:enemies" | Informational (blue text) |
| Action | "Teleported scout_02 → [5, 0, -3]" | Alert (yellow text) |
| Action | "Set collision_mask = 7 on scout_02" | Alert |
| Action | "Paused game" | Alert (bold) |
| Recording | "Querying recording wall_clip_repro, frames 4570-4590" | Informational |

Actions (state-modifying operations) are highlighted differently from passive queries. This helps the human quickly spot when the agent is changing things.

The feed shows the most recent ~20 entries. Older entries scroll off. The feed can be collapsed to save dock space.

---

## In-Game Keyboard Shortcuts

These work during gameplay (when the game is running from the editor). Handled by the `runtime.gd` autoload via `_shortcut_input()`.

| Key | Action | Behavior |
|---|---|---|
| **F8** | Toggle recording | Start recording if not active; stop if active. Visual feedback: brief flash or sound cue in-game. |
| **F9** | Drop marker | Adds a marker at the current frame. If recording is active, marker is added to the recording. Brief visual feedback (flash or corner indicator). |
| **F10** | Toggle pause | Pause or unpause the scene tree. Same as `spatial_action(action: "pause")`. Shows a visible "PAUSED" overlay when paused. |

### Input Priority

These shortcuts use `_shortcut_input()` which fires before `_unhandled_input()`. This means they take priority over game input. If a game uses F8/F9/F10 for gameplay, the user can remap Stage's shortcuts in Project Settings → Stage → Keybindings.

### Visual Feedback

When a shortcut is pressed, the runtime autoload shows brief visual feedback:
- **F8 (record start)**: Red dot appears in the corner of the game viewport, stays visible while recording
- **F8 (record stop)**: Red dot disappears with a brief flash
- **F9 (marker)**: Brief yellow flash in the corner + marker icon
- **F10 (pause)**: "PAUSED" text overlay, semi-transparent, centered on viewport

These overlays are rendered via a CanvasLayer at the highest layer index, so they appear above all game content. They're part of the runtime autoload scene, not the game scene.

---

## Agent Action Notifications

When the agent takes a state-modifying action via `spatial_action`, the human should be informed. This prevents surprise when the game state changes unexpectedly.

### Notification Types

| Action | Notification | Duration |
|---|---|---|
| `pause` | "Agent paused the game" or "Agent resumed the game" | Until dismissed or game resumes |
| `teleport` | "Agent teleported [node] to [position]" | 3 seconds |
| `set_property` | "Agent set [property] = [value] on [node]" | 3 seconds |
| `call_method` | "Agent called [method]() on [node]" | 3 seconds |
| `emit_signal` | "Agent emitted [signal] on [node]" | 3 seconds |
| `advance_frames` | "Agent advanced [N] frames" | 2 seconds |
| `advance_time` | "Agent advanced [N]s" | 2 seconds |
| `spawn_node` | "Agent spawned [scene] as [name]" | 3 seconds |
| `remove_node` | "Agent removed [node]" | 3 seconds |

### Notification Rendering

Notifications appear as toast-style popups at the top-right of the game viewport:

```
┌──────────────────────────────────────────┐
│                                          │
│          ┌───────────────────────┐       │
│          │ 🔧 Agent teleported   │       │
│          │ scout_02 → [5, 0, -3] │       │
│          └───────────────────────┘       │
│                                          │
│                                          │
│           (game viewport)                │
│                                          │
└──────────────────────────────────────────┘
```

- Semi-transparent background with readable text
- Auto-dismiss after the specified duration
- Stack vertically if multiple notifications arrive at once (max 3 visible, oldest dismissed)
- Rendered on the same CanvasLayer as shortcut feedback
- Can be disabled entirely in Project Settings → Stage → Show Agent Notifications

### Activity Log Synchronization

Agent action notifications in-game and entries in the dock's Agent Activity Feed show the same information. The in-game notifications are for when the human is focused on the game viewport; the dock feed is for when they're looking at the editor.

---

## Human-Agent Collaboration Workflow

The recording system is the primary collaboration surface. Here's the intended UX flow:

### Phase 1: Setup

**Human:**
1. Opens Godot project with Stage addon enabled
2. Sees "Waiting..." (yellow dot) in the dock — addon is listening
3. Starts their AI agent session (e.g., launches Claude Code with Stage MCP server configured)
4. Dock switches to "Connected" (green dot)
5. Agent may send initial `spatial_config` — dock's Session section updates to reflect configuration

**Agent:**
1. Discovers Stage tools are available
2. Optionally configures tracking: `spatial_config({ static_patterns: [...], state_properties: {...} })`
3. Ready for debugging

### Phase 2: Explore & Observe

**Human:**
1. Hits Play in the editor
2. Dock shows frame counter ticking, node tracking count

**Agent:**
1. Takes a snapshot: `spatial_snapshot(detail: "summary")` → gets scene overview
2. Drills into interesting areas: `spatial_snapshot(expand: "enemies")`
3. Inspects specific nodes: `spatial_inspect(node: "enemies/scout_02")`
4. Sets up watches: `spatial_watch(add: { node: "group:enemies", track: ["all"] })`

**Human sees in dock:**
- Agent Activity Feed: "Snapshot (summary)", "Expanding enemies", "Inspecting scout_02", "Watching group:enemies"

### Phase 3: Record & Reproduce

**Human:**
1. Presses F8 or clicks Record in dock — recording starts
2. Dock shows red recording indicator, timer ticking
3. Plays through the game, reproducing the bug
4. Presses F9 when the bug happens — "clipped through wall!"
5. Presses F8 to stop recording

**Agent observes:**
1. Polling `recording(action: "status")` sees recording is active
2. When recording stops, agent gets notification (or polls status)

### Phase 4: Analyze

**Agent:**
1. `recording(action: "markers")` — sees human's marker at frame 4582: "clipped through wall!"
2. `recording(action: "snapshot_at", at_frame: 4575)` — state just before the clip
3. `recording(action: "query_range", from_frame: 4570, to_frame: 4590, condition: { type: "proximity", target: "walls/*", threshold: 0.5 })` — finds the exact breach frames
4. `recording(action: "diff_frames", frame_a: 4575, frame_b: 4585)` — sees what changed
5. `recording(action: "add_marker", marker_frame: 4578, marker_label: "Root cause: collision_mask mismatch")` — annotates findings

**Human sees in dock:**
- Agent Activity Feed: "Querying recording markers", "Snapshot at frame 4575", "Query range 4570-4590", etc.
- Agent's marker appears in the recording library entry (if we show marker count)

### Phase 5: Test Fix

**Agent:**
1. `spatial_action(action: "set_property", node: "enemies/guard_01", property: "collision_mask", value: 7)`
2. `spatial_action(action: "teleport", node: "enemies/guard_01", position: [20, 0, -8])`
3. `spatial_action(action: "advance_time", seconds: 5.0, return_delta: true)`
4. Observes delta: guard stops at wall, turns around. Fix works.

**Human sees:**
- In-game notifications: "Agent set collision_mask = 7 on guard_01", "Agent teleported guard_01", "Agent advanced 5.0s"
- Dock: same actions in Agent Activity Feed
- The game visibly updates (guard moves differently)

### Phase 6: Communicate

**Agent:**
1. Reports findings to human in natural language via the chat interface
2. References specific frames, markers, and data from the analysis
3. Suggests code changes to fix the issue permanently

**Human:**
1. Reviews the agent's analysis and suggested fix
2. Can check the agent's markers in the recording
3. Applies the fix, reruns, verifies

---

## Configuration UX

Stage can be configured through three surfaces, with clear precedence:

```
Session Config (spatial_config tool)   ← Highest priority (per-session override)
       ↓
Project File (stage.toml)          ← Mid priority (version-controlled)
       ↓
Project Settings (Godot editor)        ← Lowest priority (per-machine defaults)
```

### Project Settings (Godot Editor)

Added by the EditorPlugin under **Project → Project Settings → Stage**:

| Setting | Type | Default | Description |
|---|---|---|---|
| `stage/connection/port` | int | 9077 | TCP listen port |
| `stage/connection/auto_start` | bool | true | Start TCP server automatically on Play |
| `stage/recording/storage_path` | String | `user://stage_recordings/` | Where recordings are saved |
| `stage/recording/max_frames` | int | 36000 | Safety valve (10 min at 60fps) |
| `stage/recording/capture_interval` | int | 1 | Capture every N physics frames |
| `stage/display/show_agent_notifications` | bool | true | Show in-game toast notifications |
| `stage/display/show_recording_indicator` | bool | true | Show red dot during recording |
| `stage/keybindings/toggle_recording` | InputEvent | F8 | Recording toggle key |
| `stage/keybindings/drop_marker` | InputEvent | F9 | Marker key |
| `stage/keybindings/toggle_pause` | InputEvent | F10 | Pause toggle key |
| `stage/tracking/default_static_patterns` | PackedStringArray | `[]` | Default static node patterns |
| `stage/tracking/token_hard_cap` | int | 5000 | Max tokens per response |

### Project File (stage.toml)

Optional file in the Godot project root. Version-controllable. Read by both the addon and the MCP server.

```toml
[connection]
port = 9077

[tracking]
static_patterns = ["walls/*", "terrain/*", "props/*"]
token_hard_cap = 5000

[tracking.state_properties]
enemies = ["health", "alert_level", "current_target"]
CharacterBody3D = ["velocity"]

[recording]
storage_path = "user://stage_recordings/"
max_frames = 36000
capture_interval = 1

[display]
show_agent_notifications = true
show_recording_indicator = true

[keybindings]
toggle_recording = "F8"
drop_marker = "F9"
toggle_pause = "F10"
```

### Session Config (spatial_config MCP Tool)

Overrides the above for the current MCP session. Resets when the session ends. See CONTRACT.md for the full parameter specification.

---

## Accessibility Considerations

- **Color-blind safe**: Connection status uses both color and text ("Connected" / "Disconnected"), not color alone
- **Keyboard navigable**: All dock controls are focusable and activatable via keyboard
- **Non-intrusive**: All in-game overlays can be disabled. The addon never blocks gameplay input unless paused.
- **Low-overhead UI**: Dock updates once per second (not every frame) to avoid editor performance impact

---

## Error States & Human Feedback

| Situation | Dock Indication | In-Game Indication |
|---|---|---|
| Addon loaded, no connection | Yellow dot, "Waiting on port 9077" | None |
| Connected, game not running | Green dot, "Connected (game not running)" | N/A |
| Connected, game running | Green dot, "Connected", session info visible | None (normal state) |
| Port conflict | Red text: "Port 9077 in use" | Error in Output panel |
| Connection lost | Red dot, "Disconnected — retrying..." | Brief flash if was recording |
| Recording buffer full (max_frames) | Recording auto-stops, warning shown | Red dot disappears, flash |
| Agent action fails | Error entry in Activity Feed | None |
| Game crashes during recording | Recording is lost (not flushed to disk) | N/A |

### Game Crash During Recording

If the game crashes while recording, in-memory frame data is lost. To mitigate:
- Flush frames to SQLite periodically (every 60 frames = every second)
- On crash, the partial recording is available up to the last flush
- The dock shows "Partial recording recovered" on next editor startup if applicable
