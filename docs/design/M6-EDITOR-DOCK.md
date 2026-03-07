# Design: Milestone 6 — Editor Dock

## Overview

M6 delivers the Godot editor dock panel — the human's window into what the agent is doing. The dock shows connection status, session info, and an agent activity feed. It also adds in-game toast notifications for agent actions and keyboard shortcut overlays (recording indicator, pause overlay, marker flash).

This is purely a **human-facing UI milestone**. No new MCP tools, no new Rust crates. The work is almost entirely GDScript + Godot scene files, with one small addition to the TCP protocol: the server pushes `activity_log` events to the addon so the dock can display what the agent is doing.

**Depends on:** M1 (TCP connection, snapshot flow), M4 (watch display in session info)

**Exit Criteria:** Human enables addon, sees "Waiting..." in dock. Agent connects, dock shows "Connected". Agent takes actions — dock Agent Activity Feed shows entries. Agent teleports a node — toast notification appears in game viewport. Human can follow along with what the agent is doing.

---

## Current State Analysis

### What exists (that M6 touches):

1. **plugin.gd** — `EditorPlugin` that registers Project Settings and adds `SpectatorRuntime` as an autoload singleton. Does NOT add a dock panel.

2. **runtime.gd** — Autoload that creates `SpectatorCollector` + `SpectatorTCPServer`, calls `tcp_server.poll()` every physics frame. No input handling, no overlays, no recording controls.

3. **TCP protocol** — `Message::Event` variant exists for addon → server push events (used for `signal_emitted`). No server → addon push mechanism for activity logs.

4. **Project Settings** — `spectator/display/show_agent_notifications` and `spectator/display/show_recording_indicator` are registered but not read by anything.

5. **SpectatorTCPServer** (Rust) — Exposes `start()`, `stop()`, `poll()`, `set_collector()`. No method to query connection status from GDScript. No method to get session info (watch count, tracked node count, frame number).

6. **spectator-server tcp.rs** — Read loop handles `Response`, `Error`, `Event`. Server currently only sends `Query`, `HandshakeAck`, `HandshakeError` to the addon. No `Event` push from server → addon.

### What M6 must add:

- **dock.tscn + dock.gd** — Editor dock panel scene and script
- **plugin.gd** — Add dock on `_enter_tree`, remove on `_exit_tree`
- **runtime.gd** — Keyboard shortcut handling (`_shortcut_input`), in-game overlay CanvasLayer, toast notification system
- **SpectatorTCPServer** — GDScript-callable status methods: `is_connected()`, `get_session_info()`
- **TCP protocol** — Server → addon `Event` push for activity logs
- **spectator-server** — Activity log generation in MCP tool handlers, push to addon via TCP

---

## Implementation Units

### Unit 1: Connection Status Queries (`spectator-godot`)

**File:** `crates/spectator-godot/src/tcp_server.rs`

The dock needs to query the TCP server's connection state from GDScript. Add three methods to `SpectatorTCPServer`:

```rust
#[func]
fn is_connected(&self) -> bool {
    self.handshake_completed
}

#[func]
fn get_connection_status(&self) -> GString {
    if self.handshake_completed {
        "connected".into()
    } else if self.listening {
        "waiting".into()
    } else {
        "stopped".into()
    }
}

#[func]
fn get_port(&self) -> u32 {
    self.port as u32
}
```

These read existing internal state — `handshake_completed` and `listening` are already tracked by the TCP server. No new state needed.

#### Session info query

The dock also needs session info: tracked node count, active watch count, current frame, FPS. Most of this comes from the collector and Godot engine, not the TCP server. The dock can query these directly:

- **Tracked node count**: `collector.get_tracked_count()` — new `#[func]` on `SpectatorCollector` that returns the number of nodes collected in the last frame
- **Active watch count**: Watches live on the server side, not the addon. The server pushes this as part of activity events (or the dock just shows "N/A" until the server reports it).
- **Frame number**: `Engine.get_physics_frames()` — already available in GDScript
- **FPS**: `Engine.get_frames_per_second()` — already available in GDScript

```rust
// In collector.rs
#[func]
fn get_tracked_count(&self) -> u32 {
    self.last_entity_count
}

#[func]
fn get_group_count(&self) -> u32 {
    self.last_group_count
}
```

The collector already traverses the scene tree each frame — it just needs to remember the counts from the last traversal.

**No new Rust dependencies. No protocol changes.**

---

### Unit 2: Activity Log Protocol (`spectator-protocol`, `spectator-server`, `spectator-godot`)

The server needs to push activity events to the addon so the dock can display them. This uses the existing `Message::Event` variant, but in the **server → addon** direction (currently only used addon → server).

#### Protocol: activity_log event

The server sends `Message::Event` with `event: "activity_log"`:

```json
{
  "type": "event",
  "event": "activity_log",
  "entry_type": "action",
  "summary": "Teleported scout_02 → [5.0, 0.0, -3.0]",
  "tool": "spatial_action",
  "timestamp": 1709726658.123
}
```

Entry types (matching UX.md):

| `entry_type` | Color | Description |
|---|---|---|
| `query` | Gray | Passive observation (snapshot, inspect, scene_tree, delta) |
| `watch` | Blue | Watch add/remove/list/clear |
| `action` | Yellow | State-modifying operation (pause, teleport, set_property, etc.) |
| `recording` | Blue | Recording queries |
| `config` | Gray | Configuration changes |

#### Summary generation

Each MCP tool handler generates a human-readable summary string for the activity log. Examples:

| Tool Call | Summary |
|---|---|
| `spatial_snapshot(detail: "summary")` | "Snapshot (summary)" |
| `spatial_snapshot(detail: "standard", groups: ["enemies"])` | "Snapshot (standard, groups: enemies)" |
| `spatial_snapshot(expand: "enemies")` | "Expanding cluster: enemies" |
| `spatial_inspect(node: "enemies/scout_02")` | "Inspecting enemies/scout_02" |
| `scene_tree(action: "find", find_by: "class", find_value: "CharacterBody3D")` | "Scene tree find: class=CharacterBody3D" |
| `spatial_action(action: "pause", paused: true)` | "Paused game" |
| `spatial_action(action: "teleport", node: "scout_02", position: [5,0,-3])` | "Teleported scout_02 → [5.0, 0.0, -3.0]" |
| `spatial_action(action: "set_property", node: "scout_02", property: "health", value: 50)` | "Set health = 50 on scout_02" |
| `spatial_action(action: "call_method", node: "scout_02", method: "take_damage", args: [25])` | "Called take_damage(25) on scout_02" |
| `spatial_action(action: "advance_frames", frames: 5)` | "Advanced 5 frames" |
| `spatial_action(action: "spawn_node", scene_path: "res://enemy.tscn", name: "new_enemy")` | "Spawned enemy.tscn as new_enemy" |
| `spatial_action(action: "remove_node", node: "enemies/scout_02")` | "Removed enemies/scout_02" |
| `spatial_delta()` | "Checking delta" |
| `spatial_watch(action: "add", node: "group:enemies")` | "Watching group:enemies" |
| `spatial_watch(action: "remove", watch_id: "w_1")` | "Removed watch w_1" |
| `spatial_config(bearing_format: "cardinal")` | "Config: bearing_format=cardinal" |

#### Server-side implementation

**File:** `crates/spectator-server/src/activity.rs` (new)

```rust
use serde_json::json;
use spectator_protocol::messages::Message;
use std::time::{SystemTime, UNIX_EPOCH};

/// Activity log entry types.
pub enum ActivityType {
    Query,
    Watch,
    Action,
    Recording,
    Config,
}

impl ActivityType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Watch => "watch",
            Self::Action => "action",
            Self::Recording => "recording",
            Self::Config => "config",
        }
    }
}

/// Build an activity_log Event message.
pub fn activity_event(
    entry_type: ActivityType,
    summary: &str,
    tool: &str,
) -> Message {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    Message::Event {
        event: "activity_log".to_string(),
        data: json!({
            "entry_type": entry_type.as_str(),
            "summary": summary,
            "tool": tool,
            "timestamp": timestamp,
        }),
    }
}
```

**Integration:** After each MCP tool handler completes (success or error), the server pushes an activity event to the addon. This is a fire-and-forget write — the server doesn't wait for a response.

```rust
// In each tool handler (e.g., spatial_snapshot), after the response is built:
{
    let mut s = self.state.lock().await;
    if let Some(ref mut writer) = s.tcp_writer {
        let event = activity_event(
            ActivityType::Query,
            &summary,
            "spatial_snapshot",
        );
        // Best-effort — don't fail the tool call if this write fails
        let _ = async_io::write_message(&mut writer.writer, &event).await;
    }
}
```

To avoid duplicating this pattern in every handler, add a helper method on `SpectatorServer`:

**File:** `crates/spectator-server/src/tcp.rs`

```rust
impl SessionState {
    /// Push an activity log event to the addon (best-effort, non-blocking).
    /// Call this after each MCP tool handler completes.
    pub async fn push_activity(&mut self, entry_type: &str, summary: &str, tool: &str) {
        if let Some(ref mut writer) = self.tcp_writer {
            let event = crate::activity::activity_event_from_str(entry_type, summary, tool);
            let _ = spectator_protocol::codec::async_io::write_message(
                &mut writer.writer,
                &event,
            ).await;
        }
    }
}
```

#### Addon-side: receiving activity events

**File:** `crates/spectator-godot/src/tcp_server.rs`

The TCP server's `poll()` method already reads messages. Add handling for incoming `Event` messages from the server:

```rust
Message::Event { event, data } => {
    if event == "activity_log" {
        // Store in a ring buffer for GDScript dock to read
        self.activity_log.push_back(data);
        if self.activity_log.len() > 50 {
            self.activity_log.pop_front();
        }
    }
}
```

Expose to GDScript:

```rust
#[func]
fn get_activity_log(&mut self) -> Array<Dictionary> {
    let entries: Array<Dictionary> = self.activity_log.drain(..)
        .filter_map(|v| {
            let dict = Dictionary::new();
            dict.set("entry_type", v.get("entry_type")?.as_str()?);
            dict.set("summary", v.get("summary")?.as_str()?);
            dict.set("tool", v.get("tool")?.as_str()?);
            dict.set("timestamp", v.get("timestamp")?.as_f64()?);
            Some(dict)
        })
        .collect();
    entries
}
```

The dock polls this every second (not every frame). Each call drains the buffer — entries are consumed once.

**Note on bidirectional events:** The `Message::Event` variant is currently only documented as addon → server. M6 makes it bidirectional. The addon's `poll()` read loop needs to handle `Event` messages it receives (currently it only expects `Query` from the server). This requires adding a match arm in the GDExtension's message dispatch.

---

### Unit 3: Dock Panel Scene (`addons/spectator`)

**File:** `addons/spectator/dock.tscn`

The dock is a Godot scene (`.tscn`) with a `VBoxContainer` root. Layout follows UX.md exactly.

```
dock.tscn
├── VBoxContainer (root, "SpectatorDock")
│   ├── HBoxContainer ("ConnectionSection")
│   │   ├── ColorRect ("StatusDot")  — 12x12, colored square indicator
│   │   ├── Label ("StatusLabel")      — "Connected" / "Waiting..." / "Stopped"
│   │   ├── HSeparator (spacer)
│   │   └── Label ("PortLabel")        — "Port 9077"
│   │
│   ├── HSeparator
│   │
│   ├── VBoxContainer ("SessionSection")
│   │   ├── Label ("SessionHeader")    — "Session" (bold)
│   │   ├── Label ("TrackingLabel")    — "Tracking: 47 nodes (3 groups)"
│   │   ├── Label ("WatchesLabel")     — "Watches: 2 active"
│   │   └── Label ("FrameLabel")       — "Frame: 4580 | 60 fps"
│   │
│   ├── HSeparator
│   │
│   ├── VBoxContainer ("ActivitySection")
│   │   ├── HBoxContainer ("ActivityHeader")
│   │   │   ├── Label                  — "Agent Activity" (bold)
│   │   │   └── Button ("CollapseBtn") — "▼" / "▲"
│   │   └── ScrollContainer ("ActivityScroll")
│   │       └── VBoxContainer ("ActivityList")
│   │           └── (dynamic Label children added by dock.gd)
```

**Design notes:**
- No recording controls in M6. Recording is M7. The dock structure leaves room for a recording section to be inserted between Session and Activity sections in M7.
- `ScrollContainer` has `vertical_scroll_mode = SCROLL_MODE_AUTO` and a max height of ~200px via `custom_minimum_size`.
- The dock uses Godot's built-in theme — no custom styling beyond colored text for activity entries.

---

### Unit 4: Dock Panel Script (`addons/spectator`)

**File:** `addons/spectator/dock.gd`

```gdscript
@tool
extends VBoxContainer

## Maximum number of activity entries displayed.
const MAX_ACTIVITY_ENTRIES := 20

var _tcp_server: SpectatorTCPServer
var _collector: SpectatorCollector
var _update_timer := 0.0
var _collapsed := false

# Node references (populated in _ready via unique names or get_node)
@onready var status_dot: TextureRect = %StatusDot
@onready var status_label: Label = %StatusLabel
@onready var port_label: Label = %PortLabel
@onready var tracking_label: Label = %TrackingLabel
@onready var watches_label: Label = %WatchesLabel
@onready var frame_label: Label = %FrameLabel
@onready var activity_list: VBoxContainer = %ActivityList
@onready var activity_scroll: ScrollContainer = %ActivityScroll
@onready var collapse_btn: Button = %CollapseBtn


func setup(tcp_server: SpectatorTCPServer, collector: SpectatorCollector) -> void:
    _tcp_server = tcp_server
    _collector = collector


func _ready() -> void:
    collapse_btn.pressed.connect(_toggle_collapse)
    _update_status()


func _process(delta: float) -> void:
    _update_timer += delta
    if _update_timer < 1.0:
        return
    _update_timer = 0.0
    _update_status()
    _poll_activity()


func _update_status() -> void:
    if not _tcp_server:
        _set_status("stopped", "Stopped")
        return

    var status := _tcp_server.get_connection_status()
    match status:
        "connected":
            _set_status("connected", "Connected")
        "waiting":
            _set_status("waiting", "Waiting...")
        _:
            _set_status("stopped", "Stopped")

    port_label.text = "Port %d" % _tcp_server.get_port()

    # Session info — only meaningful when a game is running
    if _collector and status == "connected":
        tracking_label.text = "Tracking: %d nodes (%d groups)" % [
            _collector.get_tracked_count(),
            _collector.get_group_count(),
        ]
        frame_label.text = "Frame: %d | %d fps" % [
            Engine.get_physics_frames(),
            Engine.get_frames_per_second(),
        ]
    else:
        tracking_label.text = "Tracking: —"
        frame_label.text = "Frame: —"

    # Watch count comes from server via activity events — we track it locally
    # when we see watch add/remove activity entries.


func _set_status(state: String, text: String) -> void:
    status_label.text = text
    match state:
        "connected":
            status_dot.modulate = Color.GREEN
        "waiting":
            status_dot.modulate = Color.YELLOW
        _:
            status_dot.modulate = Color.RED


func _poll_activity() -> void:
    if not _tcp_server:
        return

    var entries: Array[Dictionary] = _tcp_server.get_activity_log()
    for entry in entries:
        _add_activity_entry(entry)


func _add_activity_entry(entry: Dictionary) -> void:
    var label := RichTextLabel.new()
    label.bbcode_enabled = true
    label.fit_content = true
    label.scroll_active = false

    var time_str := _format_timestamp(entry.get("timestamp", 0.0))
    var summary: String = entry.get("summary", "")
    var entry_type: String = entry.get("entry_type", "query")

    var color: String
    match entry_type:
        "action":
            color = "yellow"
        "watch":
            color = "cyan"
        "recording":
            color = "cyan"
        _:
            color = "gray"

    label.text = "[color=gray]%s[/color]  [color=%s]%s[/color]" % [
        time_str, color, summary,
    ]

    activity_list.add_child(label)

    # Trim excess entries
    while activity_list.get_child_count() > MAX_ACTIVITY_ENTRIES:
        var old := activity_list.get_child(0)
        activity_list.remove_child(old)
        old.queue_free()

    # Auto-scroll to bottom
    await get_tree().process_frame
    activity_scroll.scroll_vertical = activity_scroll.get_v_scroll_bar().max_value


func _toggle_collapse() -> void:
    _collapsed = not _collapsed
    activity_scroll.visible = not _collapsed
    collapse_btn.text = "▲" if _collapsed else "▼"


static func _format_timestamp(unix: float) -> String:
    var dt := Time.get_datetime_dict_from_unix_time(int(unix))
    return "%02d:%02d:%02d" % [dt.hour, dt.minute, dt.second]
```

**Key design decisions:**

1. **Update frequency**: 1 Hz (every second), not every frame. UX.md specifies this to avoid editor performance impact.
2. **RichTextLabel for entries**: Allows colored text without custom drawing. `fit_content = true` makes each entry auto-size.
3. **Drain model**: `get_activity_log()` drains the TCP server's buffer. Entries are consumed once and live in the dock's VBoxContainer children.
4. **@tool**: Required because the dock runs in the editor, not in-game.

---

### Unit 5: Plugin Integration (`addons/spectator`)

**File:** `addons/spectator/plugin.gd`

The EditorPlugin creates the dock and wires it to the runtime.

```gdscript
@tool
extends EditorPlugin

var dock: Control


func _enter_tree() -> void:
    dock = preload("res://addons/spectator/dock.tscn").instantiate()
    add_control_to_dock(DOCK_SLOT_RIGHT_BL, dock)


func _exit_tree() -> void:
    if dock:
        remove_control_from_docks(dock)
        dock.queue_free()
        dock = null


func _enable_plugin() -> void:
    _register_settings()
    add_autoload_singleton("SpectatorRuntime", "res://addons/spectator/runtime.gd")


func _disable_plugin() -> void:
    remove_autoload_singleton("SpectatorRuntime")


func _register_settings() -> void:
    # ... existing settings registration (unchanged)
```

**Dock ↔ Runtime wiring:**

The dock lives in the editor; the runtime autoload lives in the running game. They need to share the `SpectatorTCPServer` and `SpectatorCollector` references.

Two options:

**Option A: Dock queries the autoload singleton.**

```gdscript
# In dock.gd _process:
func _update_status() -> void:
    if not _tcp_server:
        var runtime = Engine.get_singleton("SpectatorRuntime")
        if runtime:
            _tcp_server = runtime.tcp_server
            _collector = runtime.collector
```

This doesn't work — `Engine.get_singleton()` is for engine singletons, not autoloads. Autoloads are scene tree nodes, and the editor dock isn't in the game scene tree.

**Option B: Plugin passes references when they become available.**

The `plugin.gd` can listen for the autoload being added to the scene tree and connect it to the dock. But this is fragile — the autoload is added/removed on game start/stop.

**Option C (chosen): Runtime registers itself via a class-level variable.**

```gdscript
# In runtime.gd:
extends Node

## Class-level reference for dock access (set in _ready, cleared in _exit_tree).
static var instance: Node = null

var tcp_server: SpectatorTCPServer
var collector: SpectatorCollector


func _ready() -> void:
    instance = self
    # ... existing setup


func _exit_tree() -> void:
    instance = null
    # ... existing cleanup
```

```gdscript
# In dock.gd:
func _update_status() -> void:
    var runtime = SpectatorRuntime if ClassDB.class_exists(&"SpectatorRuntime") else null
    # SpectatorRuntime is a GDScript autoload, not a class — use the static var pattern
    if not _tcp_server:
        # runtime.gd sets a static var; dock reads it
        var rt_script := load("res://addons/spectator/runtime.gd")
        if rt_script and rt_script.get("instance"):
            var rt = rt_script.instance
            if rt:
                _tcp_server = rt.tcp_server
                _collector = rt.collector
```

This pattern works because:
- GDScript static vars are shared across the process
- The dock can access `runtime.gd`'s static `instance` var even though they're in different scene trees
- The static var is `null` when the game isn't running, so the dock naturally shows "Stopped"

---

### Unit 6: In-Game Overlays & Notifications (`addons/spectator`)

**File:** `addons/spectator/runtime.gd`

The runtime autoload handles keyboard shortcuts and in-game visual feedback. M6 delivers the overlay infrastructure; recording controls (F8) are wired in M7.

#### CanvasLayer setup

```gdscript
# In runtime.gd _ready(), after existing setup:
_overlay = CanvasLayer.new()
_overlay.layer = 128  # Above all game content
add_child(_overlay)

# Pause overlay (initially hidden)
_pause_label = Label.new()
_pause_label.text = "PAUSED"
_pause_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
_pause_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
_pause_label.add_theme_font_size_override("font_size", 48)
_pause_label.modulate = Color(1, 1, 1, 0.6)
_pause_label.anchors_preset = Control.PRESET_CENTER
_pause_label.visible = false
_overlay.add_child(_pause_label)

# Toast container (top-right)
_toast_container = VBoxContainer.new()
_toast_container.anchors_preset = Control.PRESET_TOP_RIGHT
_toast_container.anchor_right = 1.0
_toast_container.offset_left = -350
_toast_container.offset_top = 20
_toast_container.offset_right = -20
_overlay.add_child(_toast_container)
```

#### Keyboard shortcuts

```gdscript
func _shortcut_input(event: InputEvent) -> void:
    if not event.is_pressed() or event.is_echo():
        return

    if event is InputEventKey:
        match event.keycode:
            KEY_F10:
                _toggle_pause()
                get_viewport().set_input_as_handled()
            # F8 (recording) and F9 (marker) wired in M7
```

#### Pause toggle

```gdscript
func _toggle_pause() -> void:
    var tree := get_tree()
    tree.paused = not tree.paused
    _pause_label.visible = tree.paused
```

#### Toast notifications

Toast notifications appear when the server pushes `activity_log` events with `entry_type: "action"`. The runtime polls the same activity log as the dock and creates toast popups for action entries.

```gdscript
const MAX_TOASTS := 3
const TOAST_DURATION := 3.0

var _toasts: Array[Control] = []


func _process_toasts() -> void:
    if not tcp_server:
        return

    # Check for new action entries in the activity log
    # Note: the dock drains the buffer, so the runtime needs its own copy.
    # Solution: tcp_server exposes two methods —
    #   get_activity_log() for dock (drains all)
    #   get_action_notifications() for runtime (drains only actions)
    # OR: tcp_server stores entries and both consumers read + mark consumed.
    #
    # Simplest: tcp_server emits a Godot signal when an action event arrives.
    # The dock connects to it, the runtime connects to it. No draining needed.


func _show_toast(text: String) -> void:
    if not ProjectSettings.get_setting(
            "spectator/display/show_agent_notifications", true):
        return

    var panel := PanelContainer.new()
    panel.modulate = Color(1, 1, 1, 0.9)

    var label := Label.new()
    label.text = text
    label.autowrap_mode = TextServer.AUTOWRAP_WORD
    panel.add_child(label)

    _toast_container.add_child(panel)
    _toasts.append(panel)

    # Remove oldest if over limit
    while _toasts.size() > MAX_TOASTS:
        var old: Control = _toasts.pop_front()
        old.queue_free()

    # Auto-dismiss
    get_tree().create_timer(TOAST_DURATION).timeout.connect(func():
        if is_instance_valid(panel):
            _toasts.erase(panel)
            panel.queue_free()
    )
```

#### Signal-based activity distribution

Rather than having two consumers drain the same buffer, the TCP server emits a Godot signal when activity events arrive:

```rust
// In tcp_server.rs — add a signal
#[signal]
fn activity_received(entry_type: GString, summary: GString, tool_name: GString);
```

When an `activity_log` event is received in `poll()`:

```rust
Message::Event { event, data } if event == "activity_log" => {
    let entry_type = data.get("entry_type").and_then(|v| v.as_str()).unwrap_or("");
    let summary = data.get("summary").and_then(|v| v.as_str()).unwrap_or("");
    let tool_name = data.get("tool").and_then(|v| v.as_str()).unwrap_or("");
    self.base_mut().emit_signal(
        "activity_received",
        &[
            GString::from(entry_type).to_variant(),
            GString::from(summary).to_variant(),
            GString::from(tool_name).to_variant(),
        ],
    );
}
```

Both consumers connect:

```gdscript
# In runtime.gd _ready():
tcp_server.activity_received.connect(_on_activity_received)

func _on_activity_received(entry_type: String, summary: String, _tool: String) -> void:
    if entry_type == "action":
        _show_toast(summary)
```

```gdscript
# In dock.gd, when tcp_server reference is acquired:
_tcp_server.activity_received.connect(_on_activity_received)

func _on_activity_received(entry_type: String, summary: String, tool_name: String) -> void:
    _add_activity_entry({
        "entry_type": entry_type,
        "summary": summary,
        "tool": tool_name,
        "timestamp": Time.get_unix_time_from_system(),
    })
```

This is cleaner than a draining buffer — no coordination between consumers, no missed entries.

**Revised Unit 2:** With the signal approach, `get_activity_log()` is no longer needed. The TCP server just emits a signal. The `activity_log` ring buffer from Unit 2 is replaced by this signal.

---

### Unit 7: Server-Side Activity Logging (`spectator-server`)

**File:** `crates/spectator-server/src/activity.rs` (new)

Summary generation functions for each tool:

```rust
use super::mcp::action::SpatialActionParams;
use super::mcp::snapshot::SpatialSnapshotParams;
// ... etc

/// Generate a human-readable summary for a spatial_snapshot call.
pub fn snapshot_summary(params: &SpatialSnapshotParams) -> String {
    if let Some(ref cluster) = params.expand {
        return format!("Expanding cluster: {cluster}");
    }
    let detail = params.detail.as_deref().unwrap_or("standard");
    let mut parts = vec![format!("Snapshot ({detail})")];
    if let Some(ref groups) = params.groups {
        if !groups.is_empty() {
            parts.push(format!("groups: {}", groups.join(", ")));
        }
    }
    if let Some(ref node) = params.from_node {
        parts.push(format!("from: {node}"));
    }
    parts.join(", ")
}

/// Generate a human-readable summary for a spatial_action call.
pub fn action_summary(params: &SpatialActionParams) -> String {
    match params.action.as_str() {
        "pause" => {
            if params.paused.unwrap_or(true) {
                "Paused game".into()
            } else {
                "Resumed game".into()
            }
        }
        "advance_frames" => {
            format!("Advanced {} frames", params.frames.unwrap_or(1))
        }
        "advance_time" => {
            format!("Advanced {}s", params.seconds.unwrap_or(0.0))
        }
        "teleport" => {
            let node = params.node.as_deref().unwrap_or("?");
            let pos = params.position.as_ref()
                .map(|p| format!("{:?}", p))
                .unwrap_or_default();
            format!("Teleported {node} → {pos}")
        }
        "set_property" => {
            let node = params.node.as_deref().unwrap_or("?");
            let prop = params.property.as_deref().unwrap_or("?");
            let val = params.value.as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default();
            format!("Set {prop} = {val} on {node}")
        }
        "call_method" => {
            let node = params.node.as_deref().unwrap_or("?");
            let method = params.method.as_deref().unwrap_or("?");
            let args = params.args.as_ref()
                .or(params.method_args.as_ref())
                .map(|a| {
                    a.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
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
            let scene = params.scene_path.as_deref().unwrap_or("?")
                .rsplit('/').next().unwrap_or("?");
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

/// Generate a summary for spatial_inspect.
pub fn inspect_summary(node: &str) -> String {
    format!("Inspecting {node}")
}

/// Generate a summary for scene_tree.
pub fn scene_tree_summary(action: &str, find_by: Option<&str>, find_value: Option<&str>, node: Option<&str>) -> String {
    match action {
        "find" => {
            let by = find_by.unwrap_or("?");
            let val = find_value.unwrap_or("?");
            format!("Scene tree find: {by}={val}")
        }
        "roots" => "Scene tree: roots".into(),
        "children" => format!("Scene tree: children of {}", node.unwrap_or("root")),
        "subtree" => format!("Scene tree: subtree of {}", node.unwrap_or("root")),
        "ancestors" => format!("Scene tree: ancestors of {}", node.unwrap_or("?")),
        other => format!("Scene tree: {other}"),
    }
}

/// Generate a summary for spatial_delta.
pub fn delta_summary() -> String {
    "Checking delta".into()
}

/// Generate a summary for spatial_watch.
pub fn watch_summary(action: &str, node: Option<&str>, watch_id: Option<&str>) -> String {
    match action {
        "add" => format!("Watching {}", node.unwrap_or("?")),
        "remove" => format!("Removed watch {}", watch_id.unwrap_or("?")),
        "list" => "Listing watches".into(),
        "clear" => "Cleared all watches".into(),
        other => format!("Watch: {other}"),
    }
}

/// Generate a summary for spatial_config.
pub fn config_summary(params: &serde_json::Value) -> String {
    // Summarize which fields were set
    if let Some(obj) = params.as_object() {
        let keys: Vec<&str> = obj.keys()
            .filter(|k| !obj[*k].is_null())
            .map(|k| k.as_str())
            .collect();
        if keys.is_empty() {
            "Config: view current".into()
        } else {
            format!("Config: {}", keys.join(", "))
        }
    } else {
        "Config: view current".into()
    }
}
```

#### Wiring into tool handlers

Add a helper to `SpectatorServer` that pushes activity after each tool call:

**File:** `crates/spectator-server/src/server.rs`

```rust
impl SpectatorServer {
    /// Push an activity log event to the addon (best-effort).
    pub(crate) async fn log_activity(&self, entry_type: &str, summary: &str, tool: &str) {
        let event = crate::activity::build_activity_message(entry_type, summary, tool);
        let mut s = self.state.lock().await;
        if let Some(ref mut writer) = s.tcp_writer {
            let _ = spectator_protocol::codec::async_io::write_message(
                &mut writer.writer,
                &event,
            ).await;
        }
    }
}
```

Then at the end of each tool handler in `mod.rs`:

```rust
// spatial_snapshot — after building response:
let summary = crate::activity::snapshot_summary(&params);
self.log_activity("query", &summary, "spatial_snapshot").await;

// spatial_action — after building response:
let summary = crate::activity::action_summary(&params);
self.log_activity("action", &summary, "spatial_action").await;

// spatial_inspect:
self.log_activity("query", &crate::activity::inspect_summary(&params.node), "spatial_inspect").await;

// ... etc for all tools
```

**Important:** Activity logging happens after the response is built but before it's returned. If the tool call errors, we still log it (the summary reflects the attempt, not the outcome). This keeps the activity feed honest about what the agent tried.

---

## File Inventory

### New files

| File | Purpose |
|---|---|
| `addons/spectator/dock.tscn` | Dock panel Godot scene |
| `addons/spectator/dock.gd` | Dock panel script |
| `crates/spectator-server/src/activity.rs` | Activity summary generation |

### Modified files

| File | Changes |
|---|---|
| `addons/spectator/plugin.gd` | Add dock on `_enter_tree`, remove on `_exit_tree` |
| `addons/spectator/runtime.gd` | Add CanvasLayer overlay, keyboard shortcut handling (F10 pause), toast notifications, static `instance` var |
| `crates/spectator-godot/src/tcp_server.rs` | Add `is_connected()`, `get_connection_status()`, `get_port()`, `activity_received` signal, handle incoming `Event` messages |
| `crates/spectator-godot/src/collector.rs` | Add `get_tracked_count()`, `get_group_count()` |
| `crates/spectator-server/src/tcp.rs` | (no structural changes — activity push helper on `SessionState` or `SpectatorServer`) |
| `crates/spectator-server/src/server.rs` | Add `log_activity()` helper |
| `crates/spectator-server/src/mcp/mod.rs` | Add activity logging calls at end of each tool handler |
| `crates/spectator-server/src/main.rs` | Add `mod activity;` |

### Unchanged

| File | Reason |
|---|---|
| `crates/spectator-protocol/src/messages.rs` | `Message::Event` already supports both directions — no changes needed |
| `crates/spectator-core/` | No core logic changes — this is a UI milestone |

---

## Edge Cases & Design Decisions

### Dock lifecycle vs. game lifecycle

The dock lives in the editor (always present when addon is enabled). The game starts and stops. The dock must handle:

- **Game not running**: Status shows "Stopped" or "Waiting..." (if TCP server is listening but no game). Session info shows dashes.
- **Game starts**: Runtime sets `instance` static var. Next dock update picks it up, connects signals.
- **Game stops**: Runtime clears `instance`. TCP server is freed. Dock detects `null` references, reverts to "Stopped" state. Activity feed entries persist (they're Label children of the dock, not dependent on runtime).
- **Plugin disabled/enabled**: Dock is removed/re-added. Activity feed is cleared on re-add (fresh dock instance).

### Activity log ordering

The server pushes events after tool handler completion. Network latency between server → addon is negligible (localhost TCP). Events arrive in order. The dock appends them in order. No timestamp-based reordering needed.

### Watch count display

Watches are managed server-side. The dock can't query the watch count directly. Two approaches:

1. **Derive from activity feed**: Count `watch/add` and `watch/remove` entries to maintain a running total. Fragile if entries are missed.
2. **Server includes metadata in events**: Add an optional `meta` field to activity events. Watch events include `{ "active_watches": 3 }`. The dock reads this.

**Chosen: Option 2.** The watch summary event includes the current watch count. The dock updates its "Watches: N active" label when it sees a watch activity entry.

```json
{
  "type": "event",
  "event": "activity_log",
  "entry_type": "watch",
  "summary": "Watching group:enemies",
  "tool": "spatial_watch",
  "timestamp": 1709726658.123,
  "meta": { "active_watches": 3 }
}
```

### Performance

- Dock updates at 1 Hz — negligible editor overhead
- Activity signal emission is O(1) — just a Godot signal emit
- Toast creation is O(1) — just adding a Control child
- Maximum 20 activity labels + 3 toast panels at any time

### Accessibility

Per UX.md:
- Connection status uses both color AND text — color-blind safe
- Activity entries use colored text via BBCode, but also include the timestamp prefix for visual structure
- All dock controls are standard Godot widgets — keyboard-navigable by default
- All in-game overlays can be disabled via Project Settings

---

## Implementation Order

1. **Unit 1**: Connection status queries (Rust, small) — enables dock to show status
2. **Unit 2**: Activity log protocol (Rust, small) — TCP server handles incoming events
3. **Unit 3**: Dock scene (GDScript, layout) — static layout, no data yet
4. **Unit 4**: Dock script (GDScript, logic) — dock reads status and activity
5. **Unit 5**: Plugin integration (GDScript, small) — dock appears in editor
6. **Unit 6**: In-game overlays (GDScript, medium) — toast notifications, pause overlay, F10
7. **Unit 7**: Server activity logging (Rust, medium) — summaries generated and pushed

Units 1 and 2 are independent and can be built in parallel. Units 3-5 are sequential (scene → script → integration). Unit 6 is independent of 3-5. Unit 7 is independent of 3-6 but provides the data that makes 4 and 6 useful.

**Minimum viable demo:** Units 1 + 3 + 4 + 5 give a dock that shows connection status and session info. Units 2 + 6 + 7 add the activity feed and toast notifications.
