extends Node

var tcp_server: SpectatorTCPServer
var collector: SpectatorCollector
var recorder: SpectatorRecorder

var _overlay: CanvasLayer
var _pause_label: Label
var _toast_container: VBoxContainer
var _toasts: Array[Control] = []
var _dashcam_label: Label
var _marker_btn: Button

const MAX_TOASTS := 3
const TOAST_DURATION := 3.0

# Configurable shortcut keycodes (resolved from project settings in _ready).
var _marker_keycode: int = KEY_F9
var _pause_keycode: int = KEY_F11


func _ready() -> void:
	# Run even when the game tree is paused so TCP polling and recording continue.
	process_mode = Node.PROCESS_MODE_ALWAYS

	_resolve_shortcut_keys()

	if not ClassDB.class_exists(&"SpectatorTCPServer"):
		push_error("[Spectator] GDExtension not loaded — SpectatorTCPServer class not found. Check that the spectator.gdextension binary exists for your platform.")
		return

	var auto_start: bool = ProjectSettings.get_setting(
		"theatre/spectator/connection/auto_start", true)
	if not auto_start:
		return

	collector = SpectatorCollector.new()
	add_child(collector)

	tcp_server = SpectatorTCPServer.new()
	add_child(tcp_server)
	tcp_server.set_collector(collector)
	tcp_server.activity_received.connect(_on_activity_received)

	recorder = SpectatorRecorder.new()
	add_child(recorder)
	recorder.set_collector(collector)
	recorder.marker_added.connect(_on_marker_added)
	recorder.dashcam_clip_saved.connect(_on_dashcam_clip_saved)
	recorder.dashcam_clip_started.connect(_on_dashcam_clip_started)

	tcp_server.set_recorder(recorder)

	var port: int = 0
	var env_port := OS.get_environment("THEATRE_PORT")
	if env_port.is_empty():
		env_port = OS.get_environment("SPECTATOR_PORT")
		if not env_port.is_empty():
			push_warning("[Spectator] SPECTATOR_PORT is deprecated, use THEATRE_PORT instead")
	if not env_port.is_empty():
		port = env_port.to_int()
	if port == 0:
		port = ProjectSettings.get_setting("theatre/spectator/connection/port", 9077)
	tcp_server.start(port)
	var idle_timeout: int = ProjectSettings.get_setting(
		"theatre/spectator/connection/client_idle_timeout_secs", 10)
	tcp_server.set_idle_timeout(idle_timeout)

	_setup_overlay()

	# Push status to editor dock every 2s via EngineDebugger (only active in editor play mode).
	if EngineDebugger.is_active():
		EngineDebugger.register_message_capture("spectator", _on_debugger_command)
		var status_timer := Timer.new()
		status_timer.wait_time = 2.0
		status_timer.autostart = true
		status_timer.process_mode = Node.PROCESS_MODE_ALWAYS
		status_timer.timeout.connect(_push_status_to_editor)
		add_child(status_timer)


func _push_status_to_editor() -> void:
	if not EngineDebugger.is_active():
		return
	var status := "stopped"
	var port := 9077
	var tracked := 0
	var groups := 0
	if tcp_server:
		status = tcp_server.get_connection_status()
		port = tcp_server.get_port()
	if collector and tcp_server and tcp_server.is_connected():
		tracked = collector.get_tracked_count()
		groups = collector.get_group_count()
	EngineDebugger.send_message("spectator:status",
		[status, port, tracked, groups,
		 Engine.get_physics_frames(), Engine.get_frames_per_second()])


func _on_debugger_command(message: String, data: Array) -> bool:
	if message != "spectator:command" or data.is_empty():
		return false
	match data[0]:
		"add_marker": _drop_marker()
	return true


func _resolve_shortcut_keys() -> void:
	_marker_keycode = _key_name_to_code(ProjectSettings.get_setting(
		"theatre/spectator/shortcuts/marker_key", "F9"))
	_pause_keycode = _key_name_to_code(ProjectSettings.get_setting(
		"theatre/spectator/shortcuts/pause_key", "F11"))


static func _key_name_to_code(name: String) -> int:
	match name.to_upper().strip_edges():
		"F1": return KEY_F1
		"F2": return KEY_F2
		"F3": return KEY_F3
		"F4": return KEY_F4
		"F5": return KEY_F5
		"F6": return KEY_F6
		"F7": return KEY_F7
		"F8": return KEY_F8
		"F9": return KEY_F9
		"F10": return KEY_F10
		"F11": return KEY_F11
		"F12": return KEY_F12
	push_warning("[Spectator] Unknown shortcut key name '%s', defaulting to F12" % name)
	return KEY_F12


func _setup_overlay() -> void:
	_overlay = CanvasLayer.new()
	_overlay.layer = 128
	add_child(_overlay)

	_pause_label = Label.new()
	_pause_label.text = "⏸ PAUSED"
	_pause_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_pause_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_pause_label.add_theme_font_size_override("font_size", 48)
	_pause_label.modulate = Color(1.0, 1.0, 1.0, 0.7)
	_pause_label.set_anchors_preset(Control.PRESET_CENTER)
	_pause_label.visible = false
	_overlay.add_child(_pause_label)

	_toast_container = VBoxContainer.new()
	_toast_container.set_anchors_preset(Control.PRESET_TOP_RIGHT)
	_toast_container.anchor_left = 1.0
	_toast_container.anchor_right = 1.0
	_toast_container.offset_left = -350
	_toast_container.offset_top = 20
	_toast_container.offset_right = -20
	_overlay.add_child(_toast_container)

	_dashcam_label = Label.new()
	_dashcam_label.add_theme_font_size_override("font_size", 12)
	_dashcam_label.modulate = Color(0.6, 0.9, 1.0, 0.85)
	_dashcam_label.set_anchors_preset(Control.PRESET_TOP_LEFT)
	_dashcam_label.offset_left = 32
	_dashcam_label.offset_top = 8
	_dashcam_label.visible = false
	_overlay.add_child(_dashcam_label)

	# In-game marker button (works when dock buttons can't — dock runs in editor process).
	_marker_btn = Button.new()
	_marker_btn.text = "⚑"
	_marker_btn.tooltip_text = "Drop marker / save dashcam clip"
	_marker_btn.custom_minimum_size = Vector2(32, 32)
	_marker_btn.set_anchors_preset(Control.PRESET_TOP_LEFT)
	_marker_btn.offset_left = 10
	_marker_btn.offset_top = 32
	_marker_btn.modulate = Color(1.0, 1.0, 1.0, 0.7)
	_marker_btn.pressed.connect(_drop_marker)
	_overlay.add_child(_marker_btn)


var _dashcam_label_tick: int = 0

func _physics_process(_delta: float) -> void:
	if tcp_server:
		tcp_server.poll()
	# Update dashcam status label every ~60 frames (≈1 s at 60 fps).
	_dashcam_label_tick += 1
	if _dashcam_label_tick >= 60:
		_dashcam_label_tick = 0
		_update_dashcam_label()


func _shortcut_input(event: InputEvent) -> void:
	if not event.is_pressed() or event.is_echo():
		return
	if event is InputEventKey:
		var code: int = event.keycode
		if code == _marker_keycode:
			_drop_marker()
			get_viewport().set_input_as_handled()
		elif code == _pause_keycode:
			_toggle_pause()
			get_viewport().set_input_as_handled()


func _toggle_pause() -> void:
	var tree := get_tree()
	tree.paused = not tree.paused
	if _pause_label:
		_pause_label.visible = tree.paused


func _drop_marker() -> void:
	if not recorder:
		return
	if recorder.is_dashcam_active():
		var clip_id: String = recorder.flush_dashcam_clip("human")
		if not clip_id.is_empty():
			_show_toast("Dashcam clip saved")


## Place a code marker at the current frame.
## Tier controls dashcam behavior:
##   "system"     — rate-limited clip trigger (default, safe in loops)
##   "deliberate" — always triggers a clip (use for rare, important events)
##   "silent"     — annotates only, no clip trigger
func marker(label: String, tier: String = "system") -> void:
	if not recorder:
		return
	recorder.add_code_marker(label, tier)


func _update_dashcam_label() -> void:
	if not recorder or not _dashcam_label:
		return
	var state: String = recorder.get_dashcam_state()
	if state == "disabled":
		_dashcam_label.visible = false
		return
	var kb: int = recorder.get_dashcam_buffer_kb()
	var mb_str: String = "%.1f MB" % (kb / 1024.0)
	if state == "buffering":
		_dashcam_label.text = "● Dashcam: buffering (%s)" % mb_str
	elif state == "post_capture":
		_dashcam_label.text = "◉ Dashcam: saving clip…"
	_dashcam_label.visible = true


func _on_marker_added(_frame: int, source: String, label: String) -> void:
	var text := "Marker: %s" % label if not label.is_empty() else "Marker added"
	if source != "human":
		text = "[%s] %s" % [source, text]
	_show_toast(text)


func _on_dashcam_clip_saved(_clip_id: String, tier: String, frames: int) -> void:
	_show_toast("[dashcam] Clip saved (%s, %d frames)" % [tier, frames])
	_update_dashcam_label()


func _on_dashcam_clip_started(_trigger_frame: int, tier: String) -> void:
	_update_dashcam_label()
	if ProjectSettings.get_setting("theatre/spectator/display/show_agent_notifications", true):
		_show_toast("[dashcam] Capturing clip (%s)…" % tier)


func _on_activity_received(entry_type: String, summary: String, tool: String, active_watches: int) -> void:
	if entry_type == "action":
		_show_toast(summary)
	if EngineDebugger.is_active():
		EngineDebugger.send_message("spectator:activity",
			[entry_type, summary, tool, active_watches])


func _show_toast(text: String) -> void:
	if not ProjectSettings.get_setting("theatre/spectator/display/show_agent_notifications", true):
		return
	if not _toast_container:
		return

	var panel := PanelContainer.new()
	panel.modulate = Color(1.0, 1.0, 1.0, 0.9)

	var label := Label.new()
	label.text = text
	label.autowrap_mode = TextServer.AUTOWRAP_WORD
	panel.add_child(label)

	_toast_container.add_child(panel)
	_toasts.append(panel)

	# Remove oldest if over limit
	while _toasts.size() > MAX_TOASTS:
		var old: Control = _toasts.pop_front()
		if is_instance_valid(old):
			old.queue_free()

	# Auto-dismiss
	get_tree().create_timer(TOAST_DURATION).timeout.connect(func() -> void:
		if is_instance_valid(panel):
			_toasts.erase(panel)
			panel.queue_free()
	)


func _exit_tree() -> void:
	if tcp_server:
		tcp_server.stop()
