---
name: godot-addon
description: Working with the Godot addon GDScript layer in addons/stage/. Covers EditorPlugin patterns, autoload management, dock panels, input handling, and GDExtension integration from the GDScript side.
---

# Godot Addon — GDScript Layer

This skill covers `addons/stage/` and its shared GDScript support payload:

- `plugin.gd` — `@tool` EditorPlugin for the dock, debugger bridge, settings, and autoload registration.
- `runtime.gd` — game autoload for GDExtension lifecycle, input, overlays, markers, and runtime feedback.
- `runtime_logger.gd` — native Logger that retains bounded current-process diagnostics.
- `dock.gd` and `debugger_plugin.gd` — editor UI and editor-to-game debugger messages.
- `addons/theatre_shared/` — shared feedback producer and composer used by Stage and Director. It is not a third plugin.

## Plugin Structure

```
addons/stage/
├── plugin.cfg            # Required metadata
├── plugin.gd             # @tool EditorPlugin
├── runtime.gd            # Autoload singleton script
├── dock.tscn             # Dock panel scene
├── dock.gd               # Dock panel script
├── runtime_logger.gd     # Bounded native Logger
├── stage.gdextension     # GDExtension manifest
└── bin/                  # Compiled Rust libraries
    ├── linux/libstage_godot.so
    ├── windows/stage_godot.dll
    └── macos/libstage_godot.dylib
```

`plugin.cfg`:
```ini
[plugin]
name="Stage"
description="Spatial debugging for AI agents"
author="Your Name"
version="0.1.0"
script="plugin.gd"
```

## EditorPlugin — `plugin.gd`

```gdscript
@tool
extends EditorPlugin

var dock: Control

func _enter_tree() -> void:
    # Dock setup goes in _enter_tree (not _enable_plugin)
    dock = preload("res://addons/stage/dock.tscn").instantiate()
    add_control_to_dock(DOCK_SLOT_RIGHT_BL, dock)

func _exit_tree() -> void:
    if dock:
        remove_control_from_docks(dock)
        dock.queue_free()
        dock = null

func _enable_plugin() -> void:
    # Autoload + settings go in _enable_plugin (fires on explicit enable)
    add_autoload_singleton("StageRuntime", "res://addons/stage/runtime.gd")

func _disable_plugin() -> void:
    remove_autoload_singleton("StageRuntime")
```

**CRITICAL: Use `_enable_plugin` / `_disable_plugin` for autoload management** (fires on explicit enable, avoiding timing issues). Use `_enter_tree` / `_exit_tree` for dock setup (fires every time the plugin loads).

**Dock slot options:**
```gdscript
DOCK_SLOT_LEFT_UL   # Left sidebar, upper-left tab
DOCK_SLOT_LEFT_BL   # Left sidebar, bottom-left tab
DOCK_SLOT_RIGHT_UL  # Right sidebar, upper-left tab
DOCK_SLOT_RIGHT_BL  # Right sidebar, bottom-left tab (used by Stage)
# add_control_to_bottom_panel(control, title) for the bottom bar
```

## Autoload — `runtime.gd`

The autoload is the runtime hub. It instantiates GDExtension classes and acts as the bridge between them and the scene tree:

```gdscript
extends Node

# GDExtension instances stay untyped so a missing platform binary does not
# create a parse-time failure.
var collector
var tcp_server
var recorder

func _ready() -> void:
    if not ClassDB.class_exists(&"StageTCPServer"):
        push_error("Stage GDExtension is unavailable")
        return

    collector = ClassDB.instantiate(&"StageCollector")
    add_child(collector)

    tcp_server = ClassDB.instantiate(&"StageTCPServer")
    add_child(tcp_server)
    tcp_server.start(ProjectSettings.get_setting(
        "theatre/stage/connection/port", 9077
    ))

    recorder = ClassDB.instantiate(&"StageRecorder")
    add_child(recorder)

func _physics_process(_delta: float) -> void:
    # Pump the TCP server each physics frame (non-blocking)
    tcp_server.poll()

func _shortcut_input(event: InputEvent) -> void:
    if not event is InputEventKey or not event.pressed:
        return
    match event.keycode:
        KEY_F9: _drop_marker()
        KEY_F11: _toggle_pause()

func _drop_marker() -> void:
    # Flush dashcam clip — captures the ring buffer around this moment
    recorder.flush_dashcam_clip("human")

func _toggle_pause() -> void:
    get_tree().paused = not get_tree().paused
```

### Runtime diagnostics

`runtime.gd` registers `runtime_logger.gd` during `_init`, before the autoload's
`_ready`. The Logger callback can run on worker threads, so it only copies bounded
plain data under a mutex. It never traverses the scene tree, writes to the network,
or logs recursively. `StageTCPServer` reads retained entries on the Godot main
thread and attaches the engine-owned run identity.

Registration cannot recover earlier engine initialization output. Disabled log
streams and release builds without backtraces remain explicit limitations.

### StageRuntime.marker() — Code Markers API

Game scripts can place markers directly in code using the `StageRuntime` autoload:

```gdscript
# System tier (default) — rate-limited, safe in loops
StageRuntime.marker("player_hit")

# Deliberate tier — always triggers a clip (use for rare, important events)
StageRuntime.marker("boss_defeated", "deliberate")

# Silent tier — annotates only, no clip trigger; attached to the next clip
StageRuntime.marker("entered_zone_b", "silent")
```

**Signature:** `func marker(label: String, tier: String = "system") -> void`

- No-op when Stage is not loaded (safe to leave in shipped builds)
- Delegates to `StageRecorder.add_code_marker(label, tier)` (GDExtension export)
- Markers appear in clip data with `source: "code"`

**StageRecorder.add_code_marker(label: GString, tier: GString)** — the underlying GDExtension export:
- `"system"` tier: rate-limited dashcam trigger (2 s minimum interval)
- `"deliberate"` tier: always triggers a clip, no rate limit
- `"silent"` tier: stores in pending list; merged into the next clip whose frame range includes it; cap of 1000 pending entries with FIFO eviction

## GDExtension Classes from GDScript

GDExtension classes defined in `stage-godot` (Rust) appear as regular GDScript classes after the `.gdextension` is loaded. No import needed — they're globally available by class name:

```gdscript
# These are Rust classes, used just like built-in Godot classes
var collector = StageCollector.new()
var result: Array = collector.get_visible_nodes()
var state: Dictionary = collector.get_node_state("enemies/scout_02")
```

**GDExtension loads automatically** from the `.gdextension` manifest file. Unlike regular plugins (which need Project Settings → Plugins → Enable), GDExtension libraries load whenever the `.gdextension` file is present in the project. There is no separate enable step.

**The hybrid limitation (godot#85268):** GDScript cannot `extends` a GDExtension-derived class that itself extends `EditorPlugin`. This is why `plugin.gd` is a pure GDScript EditorPlugin that *uses* GDExtension classes, rather than extending a Rust EditorPlugin. Never attempt:
```gdscript
# WRONG — will fail to load as editor plugin
extends StageRustEditorPlugin  # if StageRustEditorPlugin extends EditorPlugin via GDExtension
```

## Input Handling

Use `_shortcut_input` (preferred over `_unhandled_input` for keyboard shortcuts):

```gdscript
func _shortcut_input(event: InputEvent) -> void:
    if not event is InputEventKey:
        return
    if not event.pressed:
        return  # ignore key release events

    match event.keycode:
        KEY_F8:
            handle_f8()
            get_viewport().set_input_as_handled()  # consume event
```

**Input method priority (earliest to latest):**
1. `_input` — catches everything first
2. `_shortcut_input` — for keyboard shortcuts, after `_input`
3. `_gui_input` — for UI nodes
4. `_unhandled_input` — what's left after UI

Use `_shortcut_input` for Stage's hotkeys to intercept before the game does.

**Consuming input:** Call `get_viewport().set_input_as_handled()` if you don't want the game to also respond to the key.

## Dock Panel — `dock.gd`

The dock runs in the editor process. It cannot access the running game's
`/root/StageRuntime`, even while Play mode is active. Use the debugger bridge:
`addons/stage/debugger_plugin.gd` forwards `stage:status` messages to the dock's
`receive_status` method and `stage:activity` messages to `receive_activity`.

```gdscript
extends VBoxContainer

@onready var status_label: Label = $StatusBar/StatusLabel

# Called by the editor debugger bridge, not by polling the game's scene tree.
func receive_status(status_text: String, _port: int, _tracked: int,
        _groups: int, _frame: int, _fps: float) -> void:
    status_label.text = status_text
```

**Accessing the autoload from a running-game script:**

```gdscript
var runtime = get_node_or_null("/root/StageRuntime")
if runtime:
    var connected: bool = runtime.tcp_server.has_stage_connection()
```

Editor-to-game commands require an attached debugger session and a matching
runtime message handler. Do not replace that boundary with editor-side autoload
lookups: those lookups inspect the editor tree, not the game tree.

## Project Settings

Register custom project settings from the EditorPlugin:

```gdscript
func _enable_plugin() -> void:
    _add_setting("theatre/stage/connection/port", TYPE_INT, 9077)
    _add_setting("theatre/stage/connection/auto_start", TYPE_BOOL, true)
    _add_setting("theatre/stage/display/show_agent_notifications", TYPE_BOOL, true)

func _add_setting(name: String, type: int, default_value) -> void:
    if not ProjectSettings.has_setting(name):
        ProjectSettings.set_setting(name, default_value)
    ProjectSettings.set_initial_value(name, default_value)
    # Makes it show in the Project Settings editor UI
    ProjectSettings.add_property_info({
        "name": name,
        "type": type,
    })
```

Read settings from any script:
```gdscript
var port: int = ProjectSettings.get_setting("theatre/stage/connection/port", 9077)
```

## Human Feedback

`runtime.gd` preloads the shared feedback producer and composer. The **Share
feedback** button and its distinct shortcut synchronously copy root-viewport
pixels and pointer context before opening the composer. Queuing is deliberate and
separate from markers and the recorder. It does not pause gameplay.

The Director editor integration uses the same support payload with the active 2D
or 3D scene viewport and current selection. Keep the payload lifecycle-free: both
existing addons can use it, but it must never register as a third EditorPlugin.
Publication uses a temporary item directory followed by rename. Do not replace
this with a queue index, a live engine dependency, or silent consumption.

## In-Game Overlay (CanvasLayer)

For pause state, dashcam status, markers, feedback, and agent activity notifications, add a CanvasLayer to the autoload:

```gdscript
# In runtime.gd _ready()
var overlay = CanvasLayer.new()
overlay.layer = 100  # Draw above everything
add_child(overlay)

var notification_label = Label.new()
overlay.add_child(notification_label)
notification_label.visible = false

func show_notification(text: String, duration: float = 3.0) -> void:
    notification_label.text = text
    notification_label.visible = true
    await get_tree().create_timer(duration).timeout
    notification_label.visible = false
```

## Common Gotchas

**`_enter_tree` autoload timing bug:** When Godot starts and the plugin is already enabled, autoloads added in `_enter_tree` aren't immediately ready. Always use `_enable_plugin` for `add_autoload_singleton`.

**GDExtension loads silently:** If the `.gdextension` binary is missing or wrong platform, Godot shows an error in the Output panel and the classes are unavailable. The plugin won't crash — GDExtension classes just won't exist. Check `ClassDB.class_exists("StageCollector")` to detect this.

**`@tool` is required:** Without `@tool` at the top of `plugin.gd`, the EditorPlugin lifecycle methods (`_enable_plugin`, `_disable_plugin`) won't run in the editor.

**Dock scenes must be freed:** If `_enter_tree()` instantiates a dock scene,
remove and free it in `_exit_tree()`. Otherwise the Control can survive plugin
reloads inside the editor.

**Autoload path:** Autoloads live at `/root/AutoloadName`. Access via `get_node("/root/StageRuntime")` or just `StageRuntime` (global alias, only works at game runtime, not in `@tool` editor code).
