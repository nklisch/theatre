---
name: godot-addon
description: Working with the Godot addon GDScript layer in addons/spectator/. Covers EditorPlugin patterns, autoload management, dock panels, input handling, and GDExtension integration from the GDScript side.
---

# Godot Addon — GDScript Layer

This skill covers `addons/spectator/` — the GDScript side of Spectator. There are three GDScript files:
- `plugin.gd` — `@tool` EditorPlugin (dock, autoload registration)
- `runtime.gd` — Autoload singleton (instantiates GDExtension classes, input handling)
- `dock.gd` — Dock panel script

## Plugin Structure

```
addons/spectator/
├── plugin.cfg            # Required metadata
├── plugin.gd             # @tool EditorPlugin
├── runtime.gd            # Autoload singleton script
├── dock.tscn             # Dock panel scene
├── dock.gd               # Dock panel script
├── spectator.gdextension # GDExtension manifest
└── bin/                  # Compiled Rust libraries
    ├── linux/libspectator_godot.so
    ├── windows/spectator_godot.dll
    └── macos/libspectator_godot.dylib
```

`plugin.cfg`:
```ini
[plugin]
name="Spectator"
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

func _enable_plugin() -> void:
    # Use _enable_plugin, NOT _enter_tree, for autoload management
    # _enter_tree has a timing bug: autoloads aren't immediately
    # accessible when the editor starts with plugin already enabled
    add_autoload_singleton("SpectatorRuntime", "res://addons/spectator/runtime.gd")

    dock = preload("res://addons/spectator/dock.tscn").instantiate()
    add_control_to_dock(DOCK_SLOT_RIGHT_UL, dock)

func _disable_plugin() -> void:
    # Always pair with _enable_plugin
    remove_autoload_singleton("SpectatorRuntime")

    if dock:
        remove_control_from_docks(dock)
        dock.queue_free()
        dock = null
```

**CRITICAL: Use `_enable_plugin` / `_disable_plugin` instead of `_enter_tree` / `_exit_tree` for autoload management.** The `_enter_tree` version has a known race condition when the editor starts with the plugin already enabled — the autoload isn't immediately accessible to other code. `_enable_plugin` fires only when the user explicitly enables the plugin, avoiding this.

**Dock slot options:**
```gdscript
DOCK_SLOT_LEFT_UL   # Left sidebar, upper-left tab
DOCK_SLOT_LEFT_BL   # Left sidebar, bottom-left tab
DOCK_SLOT_RIGHT_UL  # Right sidebar, upper-left tab (default for Spectator)
DOCK_SLOT_RIGHT_BL  # Right sidebar, bottom-left tab
# add_control_to_bottom_panel(control, title) for the bottom bar
```

## Autoload — `runtime.gd`

The autoload is the runtime hub. It instantiates GDExtension classes and acts as the bridge between them and the scene tree:

```gdscript
extends Node

# GDExtension class instances (defined in spectator-godot Rust crate)
var collector: SpectatorCollector
var tcp_server: SpectatorTCPServer
var recorder: SpectatorRecorder

func _ready() -> void:
    collector = SpectatorCollector.new()
    add_child(collector)

    tcp_server = SpectatorTCPServer.new()
    add_child(tcp_server)
    tcp_server.start(ProjectSettings.get_setting(
        "theatre/spectator/connection/port", 9077
    ))

    recorder = SpectatorRecorder.new()
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
    recorder.flush_dashcam_clip("human marker")

func _toggle_pause() -> void:
    get_tree().paused = not get_tree().paused
```

### SpectatorRuntime.marker() — Code Markers API

Game scripts can place markers directly in code using the `SpectatorRuntime` autoload:

```gdscript
# System tier (default) — rate-limited, safe in loops
SpectatorRuntime.marker("player_hit")

# Deliberate tier — always triggers a clip (use for rare, important events)
SpectatorRuntime.marker("boss_defeated", "deliberate")

# Silent tier — annotates only, no clip trigger; attached to the next clip
SpectatorRuntime.marker("entered_zone_b", "silent")
```

**Signature:** `func marker(label: String, tier: String = "system") -> void`

- No-op when Spectator is not loaded (safe to leave in shipped builds)
- Delegates to `SpectatorRecorder.add_code_marker(label, tier)` (GDExtension export)
- Markers appear in clip data with `source: "code"`

**SpectatorRecorder.add_code_marker(label: GString, tier: GString)** — the underlying GDExtension export:
- `"system"` tier: rate-limited dashcam trigger (2 s minimum interval)
- `"deliberate"` tier: always triggers a clip, no rate limit
- `"silent"` tier: stores in pending list; merged into the next clip whose frame range includes it; cap of 1000 pending entries with FIFO eviction

## GDExtension Classes from GDScript

GDExtension classes defined in `spectator-godot` (Rust) appear as regular GDScript classes after the `.gdextension` is loaded. No import needed — they're globally available by class name:

```gdscript
# These are Rust classes, used just like built-in Godot classes
var collector = SpectatorCollector.new()
var result: Array = collector.get_visible_nodes()
var state: Dictionary = collector.get_node_state("enemies/scout_02")
```

**GDExtension loads automatically** from the `.gdextension` manifest file. Unlike regular plugins (which need Project Settings → Plugins → Enable), GDExtension libraries load whenever the `.gdextension` file is present in the project. There is no separate enable step.

**The hybrid limitation (godot#85268):** GDScript cannot `extends` a GDExtension-derived class that itself extends `EditorPlugin`. This is why `plugin.gd` is a pure GDScript EditorPlugin that *uses* GDExtension classes, rather than extending a Rust EditorPlugin. Never attempt:
```gdscript
# WRONG — will fail to load as editor plugin
extends SpectatorRustEditorPlugin  # if SpectatorRustEditorPlugin extends EditorPlugin via GDExtension
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

Use `_shortcut_input` for Spectator's hotkeys to intercept before the game does.

**Consuming input:** Call `get_viewport().set_input_as_handled()` if you don't want the game to also respond to the key.

## Dock Panel — `dock.gd`

```gdscript
extends Control

@onready var connection_label: Label = $ConnectionStatus/Label
@onready var record_btn: Button = $Recording/RecordButton
@onready var stop_btn: Button = $Recording/StopButton
@onready var marker_btn: Button = $Recording/MarkerButton
@onready var timer_label: Label = $Recording/Timer
@onready var activity_list: ItemList = $AgentActivity/List

func _ready() -> void:
    record_btn.pressed.connect(_on_record_pressed)
    stop_btn.pressed.connect(_on_stop_pressed)
    marker_btn.pressed.connect(_on_marker_pressed)

    # Update UI every second (not every frame — reduce overhead)
    var timer = Timer.new()
    timer.wait_time = 1.0
    timer.timeout.connect(_update_ui)
    add_child(timer)
    timer.start()

func _update_ui() -> void:
    # Access the autoload singleton by name
    var runtime = get_node_or_null("/root/SpectatorRuntime")
    if not runtime:
        connection_label.text = "No runtime"
        return

    var is_connected: bool = runtime.tcp_server.is_connected()
    connection_label.text = "Connected" if is_connected else "Disconnected"

func _on_save_clip_pressed() -> void:
    var runtime = get_node("/root/SpectatorRuntime")
    var clip_id: String = runtime.recorder.flush_dashcam_clip("manual save")
    if not clip_id.is_empty():
        _update_clip_ui(clip_id)

func add_activity_entry(text: String) -> void:
    activity_list.add_item(text)
    if activity_list.item_count > 20:
        activity_list.remove_item(0)   # keep most recent 20
    activity_list.ensure_current_is_visible()
```

**Accessing the autoload from the dock:**
```gdscript
# Safe — returns null if not found
var runtime = get_node_or_null("/root/SpectatorRuntime")

# Panics if not found — only use when certain it exists
var runtime = get_node("/root/SpectatorRuntime")

# Or use the autoload global name directly (only works at runtime, not in editor)
SpectatorRuntime.tcp_server.is_connected()
```

The dock runs inside the editor, so accessing `SpectatorRuntime` only works when the game is running (Play mode). Use `get_node_or_null` and check for null.

## Project Settings

Register custom project settings from the EditorPlugin:

```gdscript
func _enable_plugin() -> void:
    _add_setting("theatre/spectator/connection/port", TYPE_INT, 9077)
    _add_setting("theatre/spectator/connection/auto_start", TYPE_BOOL, true)
    _add_setting("theatre/spectator/display/show_agent_notifications", TYPE_BOOL, true)

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
var port: int = ProjectSettings.get_setting("theatre/spectator/connection/port", 9077)
```

## In-Game Overlay (CanvasLayer)

For F8/F9/F10 visual feedback and agent action notifications, add a CanvasLayer to the autoload:

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

**GDExtension loads silently:** If the `.gdextension` binary is missing or wrong platform, Godot shows an error in the Output panel and the classes are unavailable. The plugin won't crash — GDExtension classes just won't exist. Check `ClassDB.class_exists("SpectatorCollector")` to detect this.

**`@tool` is required:** Without `@tool` at the top of `plugin.gd`, the EditorPlugin lifecycle methods (`_enable_plugin`, `_disable_plugin`) won't run in the editor.

**Dock scenes must be freed:** If you `instantiate()` a scene for the dock, you must `queue_free()` it in `_disable_plugin`. Forgetting leaks the Control node into the editor.

**Autoload path:** Autoloads live at `/root/AutoloadName`. Access via `get_node("/root/SpectatorRuntime")` or just `SpectatorRuntime` (global alias, only works at game runtime, not in `@tool` editor code).
