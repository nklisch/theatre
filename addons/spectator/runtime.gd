extends Node

## Class-level reference for dock access (set in _ready, cleared in _exit_tree).
static var instance: Node = null

var tcp_server: SpectatorTCPServer
var collector: SpectatorCollector
var recorder: SpectatorRecorder

var _overlay: CanvasLayer
var _pause_label: Label
var _toast_container: VBoxContainer
var _toasts: Array[Control] = []
var _recording_dot: ColorRect
var _dashcam_label: Label

const MAX_TOASTS := 3
const TOAST_DURATION := 3.0


func _ready() -> void:
	instance = self
	# Run even when the game tree is paused so TCP polling and recording continue.
	process_mode = Node.PROCESS_MODE_ALWAYS

	if not ClassDB.class_exists(&"SpectatorTCPServer"):
		push_error("[Spectator] GDExtension not loaded — SpectatorTCPServer class not found. Check that the spectator.gdextension binary exists for your platform.")
		return

	var auto_start: bool = ProjectSettings.get_setting(
		"spectator/connection/auto_start", true)
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
	recorder.recording_started.connect(_on_recording_started)
	recorder.recording_stopped.connect(_on_recording_stopped)
	recorder.marker_added.connect(_on_marker_added)
	recorder.dashcam_clip_saved.connect(_on_dashcam_clip_saved)
	recorder.dashcam_clip_started.connect(_on_dashcam_clip_started)

	tcp_server.set_recorder(recorder)

	var port: int = 0
	var env_port := OS.get_environment("SPECTATOR_PORT")
	if not env_port.is_empty():
		port = env_port.to_int()
	if port == 0:
		port = ProjectSettings.get_setting("spectator/connection/port", 9077)
	tcp_server.start(port)

	_setup_overlay()


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

	_recording_dot = ColorRect.new()
	_recording_dot.color = Color(0.9, 0.1, 0.1)
	_recording_dot.custom_minimum_size = Vector2(16, 16)
	_recording_dot.set_anchors_preset(Control.PRESET_TOP_LEFT)
	_recording_dot.offset_left = 10
	_recording_dot.offset_top = 10
	_recording_dot.visible = false
	_overlay.add_child(_recording_dot)

	_dashcam_label = Label.new()
	_dashcam_label.add_theme_font_size_override("font_size", 12)
	_dashcam_label.modulate = Color(0.6, 0.9, 1.0, 0.85)
	_dashcam_label.set_anchors_preset(Control.PRESET_TOP_LEFT)
	_dashcam_label.offset_left = 32
	_dashcam_label.offset_top = 8
	_dashcam_label.visible = false
	_overlay.add_child(_dashcam_label)


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
		match event.keycode:
			KEY_F8:
				_toggle_recording()
				get_viewport().set_input_as_handled()
			KEY_F9:
				_drop_marker()
				get_viewport().set_input_as_handled()
			KEY_F10:
				_toggle_pause()
				get_viewport().set_input_as_handled()


func _toggle_pause() -> void:
	var tree := get_tree()
	tree.paused = not tree.paused
	if _pause_label:
		_pause_label.visible = tree.paused


func _toggle_recording() -> void:
	if not recorder:
		return
	if recorder.is_recording():
		recorder.stop_recording()
		_set_recording_indicator(false)
	else:
		var storage_path: String = ProjectSettings.get_setting(
			"spectator/recording/storage_path", "user://spectator_recordings/")
		var interval: int = ProjectSettings.get_setting(
			"spectator/recording/capture_interval", 1)
		var max_frames: int = ProjectSettings.get_setting(
			"spectator/recording/max_frames", 36000)
		var id: String = recorder.start_recording("", storage_path, interval, max_frames)
		if not id.is_empty():
			_set_recording_indicator(true)


func _drop_marker() -> void:
	if not recorder:
		return
	# Explicit recording takes priority for F9.
	if recorder.is_recording():
		recorder.add_marker("human", "")
		if _recording_dot:
			_recording_dot.color = Color.YELLOW
			get_tree().create_timer(0.3).timeout.connect(func() -> void:
				if _recording_dot:
					_recording_dot.color = Color(0.9, 0.1, 0.1)
			)
	elif recorder.is_dashcam_active():
		# Dashcam-only mode: flush ring buffer as a human clip.
		var clip_id: String = recorder.flush_dashcam_clip("human")
		if not clip_id.is_empty():
			_show_toast("Dashcam clip saved")


func _set_recording_indicator(visible: bool) -> void:
	if not ProjectSettings.get_setting(
			"spectator/display/show_recording_indicator", true):
		return
	if _recording_dot:
		_recording_dot.visible = visible


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


func _on_recording_started(_id: String, _name: String) -> void:
	_set_recording_indicator(true)
	_show_toast("Recording started")


func _on_recording_stopped(_id: String, _frames: int) -> void:
	_set_recording_indicator(false)


func _on_marker_added(_frame: int, source: String, label: String) -> void:
	var text := "Marker: %s" % label if not label.is_empty() else "Marker added"
	if source != "human":
		text = "[%s] %s" % [source, text]
	_show_toast(text)
	if _recording_dot:
		_recording_dot.color = Color.YELLOW
		get_tree().create_timer(0.3).timeout.connect(func() -> void:
			if _recording_dot:
				_recording_dot.color = Color(0.9, 0.1, 0.1)
		)


func _on_dashcam_clip_saved(recording_id: String, tier: String, frames: int) -> void:
	_show_toast("[dashcam] Clip saved (%s, %d frames)" % [tier, frames])
	_update_dashcam_label()
	var _ = recording_id  # used by dock library list in future


func _on_dashcam_clip_started(_trigger_frame: int, tier: String) -> void:
	_update_dashcam_label()
	if ProjectSettings.get_setting("spectator/display/show_agent_notifications", true):
		_show_toast("[dashcam] Capturing clip (%s)…" % tier)


func _on_activity_received(entry_type: String, summary: String, _tool: String, _active_watches: int) -> void:
	if entry_type == "action":
		_show_toast(summary)


func _show_toast(text: String) -> void:
	if not ProjectSettings.get_setting("spectator/display/show_agent_notifications", true):
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
	instance = null
	if recorder and recorder.is_recording():
		recorder.stop_recording()
	if tcp_server:
		tcp_server.stop()
