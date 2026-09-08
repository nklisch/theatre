extends Node

const CaptureControls := preload("res://addons/stage/capture_controls.gd")
const RuntimeLoggerScript := preload("res://addons/stage/runtime_logger.gd")
const LaunchPolicy := preload("res://addons/stage/launch_policy.gd")
const Feedback := preload("res://addons/theatre_shared/feedback.gd")
const FeedbackComposer := preload("res://addons/theatre_shared/feedback_composer.gd")
var _feedback_composer: ConfirmationDialog

var tcp_server
var collector
var recorder
var _runtime_logger: Logger
var launch_config: Dictionary = {}
var launch_error := ""
var target_scene := ""
var _project_ready := false
var _recording_id := ""
var _metrics_enabled := false
var _metric_providers: Dictionary = {}
var _metadata_providers: Dictionary = {}
var _phase := ""

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
	var result := LaunchPolicy.runtime_request()
	if not result.get("ok", false):
		launch_error = str(result.get("error", "Invalid launch policy"))
		return
	launch_config = result.config
	target_scene = result.get("target_scene", "")
	if launch_config.observe:
		_runtime_logger = RuntimeLoggerScript.new()
		OS.add_logger(_runtime_logger)


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	set_process(false)
	set_physics_process(false)
	set_process_shortcut_input(false)
	if not launch_error.is_empty():
		push_error("[Stage] " + launch_error)
		return
	if not launch_config.observe and not launch_config.startup.enabled and not launch_config.play.enabled:
		return
	_resolve_shortcut_keys()
	_setup_overlay()
	set_process_shortcut_input(true)
	set_physics_process(true)
	if launch_config.observe:
		_ensure_collector()
		tcp_server = ClassDB.instantiate(&"StageTCPServer")
		add_child(tcp_server)
		tcp_server.set_collector(collector)
		tcp_server.set_runtime_logger(_runtime_logger)
		tcp_server.activity_received.connect(_on_activity_received)
		var port := int(OS.get_environment("THEATRE_PORT"))
		if port == 0:
			port = ProjectSettings.get_setting("theatre/stage/connection/port", 9077)
		tcp_server.start(port)
		tcp_server.set_idle_timeout(ProjectSettings.get_setting("theatre/stage/connection/client_idle_timeout_secs", 10))
	if launch_config.startup.enabled or launch_config.play.enabled:
		recorder = ClassDB.instantiate(&"StageRecorder")
		recorder.capture_manage()
		recorder.set_dashcam_enabled(false)
		add_child(recorder)
		recorder.marker_added.connect(_on_marker_added)
		if tcp_server:
			tcp_server.set_recorder(recorder)
		if launch_config.startup.enabled:
			if target_scene.is_empty():
				launch_error = "Startup capture requires the Theatre bootstrap and an explicit target scene."
				push_error("[Stage] " + launch_error)
			else:
				_begin_phase("startup", false)
		if launch_config.readiness == "scene":
			get_tree().scene_changed.connect(_on_scene_changed)
			call_deferred("_on_scene_changed")
	_update_capture_controls()
	if launch_config.observe and EngineDebugger.is_active():
		EngineDebugger.register_message_capture("stage", _on_debugger_command)
		var status_timer := Timer.new()
		status_timer.wait_time = 2.0
		status_timer.autostart = true
		status_timer.process_mode = Node.PROCESS_MODE_ALWAYS
		status_timer.timeout.connect(_push_status_to_editor)
		add_child(status_timer)


func _ensure_collector() -> void:
	if collector == null:
		collector = ClassDB.instantiate(&"StageCollector")
		add_child(collector)
	if recorder != null:
		recorder.set_collector(collector)


func _on_scene_changed() -> void:
	var scene := get_tree().current_scene
	if scene == null or scene.scene_file_path == "res://addons/stage/capture_bootstrap.tscn":
		return
	if not target_scene.is_empty() and scene.scene_file_path != target_scene:
		return
	notify_ready()


## Call once when asynchronous project loading has actually finished. Safe off.
func notify_ready() -> Dictionary:
	if _project_ready or launch_config.is_empty():
		return capture_status()
	_project_ready = true
	if recorder != null and _phase == "startup":
		_capture_result(recorder.capture_stop("project_ready"))
		_metrics_enabled = false
		set_process(false)
	if recorder != null and launch_config.play.enabled and launch_config.play_start == "ready":
		# A failed startup save keeps its draft and deliberately blocks auto-play.
		if capture_status().get("state") == "idle":
			_begin_phase("play", true)
	_update_capture_controls()
	return capture_status()


func _begin_phase(phase: String, continue_recording: bool) -> Dictionary:
	if recorder == null:
		return {"error": "Capture was not enabled for this launch"}
	if phase == "play" and not _project_ready:
		return {"error": "Waiting for the configured project readiness notification"}
	var config: Dictionary = launch_config[phase]
	if not config.enabled or recorder.capture_is_active() or capture_status().get("state") == "review":
		return _start_error("Enable the next phase and finish/keep or discard the current segment before starting")
	var project_metadata := {}
	for provider_name in _metadata_providers.keys():
		var callback: Callable = _metadata_providers.get(provider_name, Callable())
		if callback.is_valid():
			var value: Variant = callback.call()
			if not value is Dictionary:
				return _start_error("Metadata providers must return a Dictionary")
			project_metadata[provider_name] = value
	var metadata_encoded := JSON.stringify(project_metadata)
	if metadata_encoded.to_utf8_buffer().size() > 64 * 1024:
		return _start_error("Project metadata exceeds 64 KiB encoded payload")
	if config.spatial:
		_ensure_collector()
	var context := {"target_scene": target_scene, "readiness": launch_config.readiness,
		"operator": launch_config.operator, "sources": launch_config.sources,
		"project_metadata": project_metadata}
	var result := _capture_result(recorder.capture_begin(JSON.stringify(config), phase,
		_recording_id if continue_recording else "", JSON.stringify(context)))
	if not result.has("error"):
		_capture_controls.clear_acknowledgement()
		_recording_id = result.recording_id
		_phase = phase
		_metrics_enabled = config.metrics
		set_process(_metrics_enabled)
	_update_capture_controls()
	return result


func capture_start(continue_recording: bool = false) -> Dictionary:
	return _begin_phase("play", continue_recording)


func _start_error(message: String) -> Dictionary:
	_acknowledge_capture(message)
	return {"error": message}


func capture_stop(reason: String = "manual_stop") -> Dictionary:
	if recorder == null:
		return {"error": "Capture is off"}
	var result := _capture_result(recorder.capture_stop(reason))
	if not result.has("error"):
		_capture_controls.clear_acknowledgement()
	_metrics_enabled = false
	set_process(false)
	_update_capture_controls()
	return result


func capture_keep(note: String = "") -> Dictionary:
	if recorder == null:
		return {"error": "Capture is off"}
	var result := _capture_result(recorder.capture_keep(note))
	if not result.has("error"):
		_acknowledge_capture("Saved · Ready for a new segment")
	_update_capture_controls()
	return result


func capture_save(note: String = "") -> Dictionary:
	if recorder == null:
		return {"error": "Capture is off"}
	if recorder.capture_is_active():
		var stopped := capture_stop()
		if stopped.has("error") or stopped.get("state") == "idle":
			return stopped
	return capture_keep(note)


func capture_discard() -> Dictionary:
	if recorder == null:
		return {"error": "Capture is off"}
	var result := _capture_result(recorder.capture_discard())
	if not result.has("error"):
		_capture_controls.clear_acknowledgement()
	_update_capture_controls()
	return result


func capture_status() -> Dictionary:
	var status: Dictionary = JSON.parse_string(recorder.capture_status()) if recorder != null else {"state": "off"}
	status["ready"] = _project_ready
	status["launch"] = launch_config
	status["target_scene"] = target_scene
	status["can_continue"] = not _recording_id.is_empty()
	status["next_config"] = launch_config.get("play", {})
	return status


func capture_configure(options: Dictionary) -> Dictionary:
	if recorder == null:
		return {"error": "Capture is off for this launch; relaunch with a capture phase enabled"}
	var result: Dictionary = JSON.parse_string(ClassDB.class_call_static(&"StageCapturePolicy", &"configure_play",
		JSON.stringify(launch_config), JSON.stringify(options)))
	if not result.get("ok", false):
		return {"error": result.error}
	launch_config = result.config
	_capture_controls.clear_acknowledgement()
	_update_capture_controls()
	return capture_status()


func _capture_result(encoded: String) -> Dictionary:
	var result: Dictionary = JSON.parse_string(encoded)
	if result.has("error"):
		_acknowledge_capture(str(result.error))
	return result


## Explicit providers only. No discovery, reflection, or callbacks while idle.
func register_metric_provider(provider_name: String, callback: Callable) -> bool:
	if provider_name.is_empty() or provider_name.length() > 64 or provider_name == "engine" or not callback.is_valid():
		return false
	if not _metric_providers.has(provider_name) and _metric_providers.size() >= 32:
		return false
	_metric_providers[provider_name] = callback
	return true


func unregister_metric_provider(provider_name: String) -> void:
	_metric_providers.erase(provider_name)


## Snapshot small project context at explicit phase/Start boundaries, never idle.
## Automatic rolling continuations retain this context rather than call providers.
func register_metadata_provider(provider_name: String, callback: Callable) -> bool:
	if provider_name.is_empty() or provider_name.length() > 64 or not callback.is_valid():
		return false
	if not _metadata_providers.has(provider_name) and _metadata_providers.size() >= 32:
		return false
	_metadata_providers[provider_name] = callback
	return true


func unregister_metadata_provider(provider_name: String) -> void:
	_metadata_providers.erase(provider_name)


func _process(delta: float) -> void:
	if recorder == null or not _metrics_enabled or not recorder.capture_is_active():
		set_process(false)
		return
	var started := Time.get_ticks_usec()
	var values := {"engine": {"process_delta_secs": delta,
		"fps": Engine.get_frames_per_second(),
		"static_memory_bytes": Performance.get_monitor(Performance.MEMORY_STATIC),
		"object_count": Performance.get_monitor(Performance.OBJECT_COUNT)}}
	for provider_name in _metric_providers.keys():
		var callback: Callable = _metric_providers.get(provider_name, Callable())
		if callback.is_valid():
			var value: Variant = callback.call()
			if value is Dictionary:
				values[provider_name] = value
	# Callback work and serialization are measured, but never claimed as total
	# observer cost: native collection/persistence have separate timings.
	var encoded := JSON.stringify(values)
	_capture_result(recorder.capture_metrics(encoded, Time.get_ticks_usec() - started))


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
	_capture_controls.continue_requested.connect(func() -> void: capture_start(true))
	_capture_controls.discard_requested.connect(func() -> void: capture_discard())
	_capture_controls.cancel_requested.connect(func() -> void: capture_configure({"preset": "off"}))
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
	if is_instance_valid(_feedback_composer) and _feedback_composer.visible:
		return
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
		_acknowledge_capture("Capture is off for this launch · Relaunch with a capture phase enabled")
	elif not recorder.capture_is_active():
		_acknowledge_capture("Capture idle · Start recording before marking")
	else:
		recorder.add_marker("human", "Human marker")


func _toggle_capture() -> void:
	if not recorder:
		return
	if recorder.capture_is_active():
		capture_stop()
	else:
		capture_start(false)


func _save_capture_now() -> void:
	capture_keep()


func _apply_capture_preset(preset: String) -> void:
	if not recorder:
		return
	var result := capture_configure({"preset": preset})
	if not result.has("error"):
		_acknowledge_capture("%s selected for the next segment" % preset.capitalize())
	else:
		_acknowledge_capture(str(result.error))
	_update_capture_controls()


## Place a code marker at the current frame.
## Tier controls dashcam behavior:
##   "system"     — rate-limited marker; triggers only with on_trigger saving
##   "deliberate" — marker; triggers only with on_trigger saving
##   "silent"     — annotates only, no clip trigger
func marker(label: String, tier: String = "system") -> void:
	if not recorder:
		return
	recorder.add_code_marker(label, tier)


func _update_capture_controls() -> void:
	if not _capture_controls:
		return
	_capture_controls.refresh(capture_status())


func _on_marker_added(frame: int, source: String, label: String) -> void:
	if source == "human":
		_acknowledge_capture("Marked frame %d" % frame)
	else:
		_show_toast("[%s] Marker: %s" % [source, label])
	call_deferred("_update_capture_controls")


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
	if recorder != null and is_instance_valid(recorder):
		var status: Dictionary = JSON.parse_string(recorder.capture_status())
		if status.get("state") == "recording" and _phase == "startup":
			recorder.capture_stop("runtime_exit_before_ready")
		elif status.get("state") in ["recording", "review"]:
			push_warning("[Stage] Unkept play capture is local memory only and is lost on exit. Stop and Keep before closing.")
	if _runtime_logger:
		OS.remove_logger(_runtime_logger)
	if tcp_server:
		tcp_server.stop()
