extends Node

const CaptureControls := preload("res://addons/stage/capture_controls.gd")
const RuntimeLoggerScript := preload("res://addons/stage/runtime_logger.gd")
const Feedback := preload("res://addons/theatre_shared/feedback.gd")
const FeedbackComposer := preload("res://addons/theatre_shared/feedback_composer.gd")
var _feedback_composer: ConfirmationDialog

var tcp_server
var collector
var recorder
var _runtime_logger: Logger

var _overlay: CanvasLayer
var _pause_label: Label
var _toast_container: VBoxContainer
var _toasts: Array[Control] = []
var _capture_controls: PanelContainer

const MAX_TOASTS := 3
const TOAST_DURATION := 3.0

# Configurable shortcut keycodes (resolved from project settings in _ready).
var _marker_keycode: int = KEY_F9
var _pause_keycode: int = KEY_F11


func _init() -> void:
	# Register before _ready so capture begins as early as the autoload lifecycle
	# permits. Godot initialization and pre-registration history are unavailable.
	_runtime_logger = RuntimeLoggerScript.new()
	OS.add_logger(_runtime_logger)


func _ready() -> void:
	# Run even when the game tree is paused so TCP polling and recording continue.
	process_mode = Node.PROCESS_MODE_ALWAYS

	_resolve_shortcut_keys()
	_setup_overlay()

	for extension_class in [&"StageTCPServer", &"StageCollector", &"StageRecorder"]:
		if not ClassDB.class_exists(extension_class):
			push_error("[Stage] GDExtension not loaded — %s class not found. Check that the stage.gdextension binary exists for your platform." % extension_class)
			return

	var auto_start: bool = ProjectSettings.get_setting(
		"theatre/stage/connection/auto_start", true)
	if not auto_start:
		return

	collector = ClassDB.instantiate(&"StageCollector")
	add_child(collector)

	tcp_server = ClassDB.instantiate(&"StageTCPServer")
	add_child(tcp_server)
	tcp_server.set_collector(collector)
	tcp_server.set_runtime_logger(_runtime_logger)
	tcp_server.activity_received.connect(_on_activity_received)

	recorder = ClassDB.instantiate(&"StageRecorder")
	var dashcam_enabled: bool = ProjectSettings.get_setting(
		"theatre/stage/dashcam/enabled", true)
	recorder.set_dashcam_enabled(dashcam_enabled)
	add_child(recorder)
	recorder.set_collector(collector)
	recorder.marker_added.connect(_on_marker_added)
	recorder.dashcam_clip_saved.connect(_on_dashcam_clip_saved)
	recorder.dashcam_clip_started.connect(_on_dashcam_clip_started)
	recorder.dashcam_clip_failed.connect(_on_dashcam_clip_failed)
	_update_capture_controls()

	tcp_server.set_recorder(recorder)

	var port: int = 0
	var env_port := OS.get_environment("THEATRE_PORT")
	if not env_port.is_empty():
		port = env_port.to_int()
	if port == 0:
		port = ProjectSettings.get_setting("theatre/stage/connection/port", 9077)
	tcp_server.start(port)
	var idle_timeout: int = ProjectSettings.get_setting(
		"theatre/stage/connection/client_idle_timeout_secs", 10)
	tcp_server.set_idle_timeout(idle_timeout)

	# Push status to editor dock every 2s via EngineDebugger (only active in editor play mode).
	if EngineDebugger.is_active():
		EngineDebugger.register_message_capture("stage", _on_debugger_command)
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
	if collector and tcp_server and tcp_server.has_stage_connection():
		tracked = collector.get_tracked_count()
		groups = collector.get_group_count()
	EngineDebugger.send_message("stage:status",
		[status, port, tracked, groups,
		 Engine.get_physics_frames(), Engine.get_frames_per_second()])


func _on_debugger_command(message: String, data: Array) -> bool:
	if message != "stage:command" or data.is_empty():
		return false
	match data[0]:
		"add_marker": _drop_marker()
	return true


func _resolve_shortcut_keys() -> void:
	_marker_keycode = _key_name_to_code(ProjectSettings.get_setting(
		"theatre/stage/shortcuts/marker_key", "F9"))
	_pause_keycode = _key_name_to_code(ProjectSettings.get_setting(
		"theatre/stage/shortcuts/pause_key", "F11"))


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
	push_warning("[Stage] Unknown shortcut key name '%s', defaulting to F12" % name)
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

	_capture_controls = CaptureControls.new()
	_overlay.add_child(_capture_controls)
	_capture_controls.configure(OS.get_keycode_string(_marker_keycode),
		str(ProjectSettings.get_setting("theatre/stage/display/capture_controls", "bottom_right")))
	_capture_controls.toggle_requested.connect(_toggle_capture)
	_capture_controls.marker_requested.connect(_drop_marker)
	_capture_controls.save_requested.connect(_save_capture_now)
	_capture_controls.preset_requested.connect(_apply_capture_preset)
	_capture_controls.feedback_requested.connect(share_feedback)
	_capture_controls.refresh({})


var _capture_status_tick: int = 0

func _physics_process(_delta: float) -> void:
	if tcp_server:
		tcp_server.poll()
	# Update capture status every ~60 frames (≈1 s at 60 fps).
	_capture_status_tick += 1
	if _capture_status_tick >= 60:
		_capture_status_tick = 0
		_update_capture_controls()


func _shortcut_input(event: InputEvent) -> void:
	if not event.is_pressed() or event.is_echo():
		return
	if event is InputEventKey:
		var code: int = event.keycode
		if code == KEY_F8 and event.ctrl_pressed and event.shift_pressed:
			share_feedback()
			get_viewport().set_input_as_handled()
		elif code == _marker_keycode:
			_drop_marker()
			get_viewport().set_input_as_handled()
		elif code == _pause_keycode:
			_toggle_pause()
			get_viewport().set_input_as_handled()


## Deliberate capture, separate from the compatible marker API and recorder.
func share_feedback() -> void:
	if is_instance_valid(_feedback_composer):
		_feedback_composer.grab_focus()
		return
	var scene := get_tree().current_scene
	var scene_path := scene.scene_file_path if scene != null else ""
	var run_id: Variant = tcp_server.get_run_id() if tcp_server != null else null
	var composition := Feedback.capture(get_tree().root, "runtime", scene_path, [], "root_viewport", run_id)
	_feedback_composer = FeedbackComposer.new()
	add_child(_feedback_composer)
	_feedback_composer.compose(composition)


func _toggle_pause() -> void:
	var tree := get_tree()
	tree.paused = not tree.paused
	if _pause_label:
		_pause_label.visible = tree.paused


func _acknowledge_capture(message: String) -> void:
	if _capture_controls:
		_capture_controls.acknowledge(message)
	if not _capture_controls or not _capture_controls.visible:
		_show_toast(message, true)


func _drop_marker() -> void:
	if not recorder:
		_acknowledge_capture("Recorder unavailable · Check Stage auto-start and addon loading")
	elif not recorder.is_dashcam_active():
		_acknowledge_capture("Dashcam stopped · Start recording before marking")
	else:
		recorder.add_marker("human", "Human marker")


func _toggle_capture() -> void:
	if not recorder:
		return
	var was_pending: bool = recorder.get_dashcam_state() == "post_capture"
	var enable: bool = not recorder.is_dashcam_active()
	recorder.set_dashcam_enabled(enable)
	var status: Dictionary = JSON.parse_string(recorder.get_dashcam_status_json())
	if status.get("last_save_error") != null:
		_acknowledge_capture(str(status.last_save_error))
	elif enable:
		_acknowledge_capture("Dashcam started · buffering gameplay")
	elif was_pending and status.get("last_saved_clip") is Dictionary:
		_acknowledge_capture("Stopped · available capture saved as %s" % status.last_saved_clip.clip_id)
	else:
		_acknowledge_capture("Dashcam stopped · no new clip saved")
	_update_capture_controls()


func _save_capture_now() -> void:
	if not recorder or not recorder.is_dashcam_active():
		_acknowledge_capture("Start dashcam before saving a clip")
		return
	if recorder.get_dashcam_buffer_frames() == 0:
		_acknowledge_capture("No sampled gameplay yet · wait for the buffer")
		return
	recorder.flush_dashcam_clip("Human Save now")
	_update_capture_controls()


func _apply_capture_preset(preset: String) -> void:
	if not recorder:
		return
	var result: Dictionary = JSON.parse_string(recorder.apply_dashcam_config(JSON.stringify({"preset":preset})))
	if result.has("error"):
		_acknowledge_capture(str(result.error))
	else:
		_acknowledge_capture("%s capture settings applied · recording state unchanged" % preset.capitalize())
	_update_capture_controls()


## Place a code marker at the current frame.
## Tier controls dashcam behavior:
##   "system"     — rate-limited clip trigger (default, safe in loops)
##   "deliberate" — always triggers a clip (use for rare, important events)
##   "silent"     — annotates only, no clip trigger
func marker(label: String, tier: String = "system") -> void:
	if not recorder:
		return
	recorder.add_code_marker(label, tier)


func _update_capture_controls() -> void:
	if not _capture_controls:
		return
	var status: Dictionary = {}
	if recorder:
		status = JSON.parse_string(recorder.get_dashcam_status_json())
	_capture_controls.refresh(status)


func _on_marker_added(frame: int, source: String, label: String) -> void:
	if source == "human":
		_acknowledge_capture("Marked frame %d · collecting the post-window" % frame)
	else:
		_show_toast("[%s] Marker: %s" % [source, label])
	call_deferred("_update_capture_controls")


func _on_dashcam_clip_saved(clip_id: String, _tier: String, _frames: int) -> void:
	_acknowledge_capture("Clip saved: %s" % clip_id)
	# Never re-enter recorder getters while its native save call is still bound.
	call_deferred("_update_capture_controls")


func _on_dashcam_clip_failed(message: String) -> void:
	_acknowledge_capture(message)
	call_deferred("_update_capture_controls")


func _on_dashcam_clip_started(_trigger_frame: int, tier: String) -> void:
	call_deferred("_update_capture_controls")
	_show_toast("[dashcam] Collecting post-window (%s)…" % tier)


func _on_activity_received(entry_type: String, summary: String, tool: String, active_watches: int) -> void:
	if entry_type == "action":
		_show_toast(summary)
	if EngineDebugger.is_active():
		EngineDebugger.send_message("stage:activity",
			[entry_type, summary, tool, active_watches])


func _show_toast(text: String, human_confirmation: bool = false) -> void:
	if not human_confirmation and not ProjectSettings.get_setting("theatre/stage/display/show_agent_notifications", true):
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
	if _runtime_logger:
		OS.remove_logger(_runtime_logger)
	if tcp_server:
		tcp_server.stop()
