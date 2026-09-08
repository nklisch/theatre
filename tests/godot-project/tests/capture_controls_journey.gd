extends SceneTree

var failures: Array[String] = []
var emitted_markers := 0

func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	OS.set_environment("THEATRE_LAUNCH_JSON", JSON.stringify({"options": {
		"readiness": "project", "play": {"preset": "light", "scope": ["."]}}}))
	ProjectSettings.set_setting("theatre/stage/display/show_agent_notifications", false)
	ProjectSettings.set_setting("theatre/stage/shortcuts/marker_key", "F7")
	root.size = Vector2i(320, 240)
	var scene := Node2D.new()
	scene.name = "CaptureJourney"
	root.add_child(scene)
	current_scene = scene
	var runtime = load("res://addons/stage/runtime.gd").new()
	runtime.name = "StageRuntime"
	root.add_child(runtime)
	call_deferred("_exercise", runtime)

func check(condition: bool, message: String) -> bool:
	if not condition:
		failures.append(message)
	return condition

func _exercise(runtime: Node) -> void:
	await process_frame
	if not check(runtime.recorder != null, "recorder initialized"):
		finish()
		return
	var controls: Control = runtime._capture_controls
	var mark: Button = controls.find_child("Mark", true, false)
	var toggle: Button = controls.find_child("ToggleDashcam", true, false)
	var keep: Button = controls.find_child("SaveNow", true, false)
	var discard: Button = controls.find_child("DiscardCapture", true, false)
	var resume: Button = controls.find_child("ContinueCapture", true, false)
	var status: Label = controls.find_child("CaptureStatus", true, false)
	var preset: OptionButton = controls.find_child("CapturePreset", true, false)
	runtime.recorder.marker_added.connect(func(_frame: int, _source: String, _label: String) -> void: emitted_markers += 1)
	check(toggle.disabled and mark.disabled and keep.disabled, "waiting cannot start before readiness")
	check(mark.text.contains("F7"), "configured shortcut displayed")
	runtime.notify_ready()
	for index in preset.item_count:
		preset.item_selected.emit(index)
		check(runtime.capture_status().next_config.preset == preset.get_item_metadata(index), "preset is staged")
		check(runtime.capture_status().state == "idle", "preset never starts capture")
	runtime._apply_capture_preset("light")
	check(toggle.focus_mode == Control.FOCUS_ALL, "capture actions support keyboard focus")
	toggle.grab_focus()
	var accept := InputEventAction.new()
	accept.action = "ui_accept"
	accept.pressed = true
	Input.parse_input_event(accept)
	await process_frame
	accept.pressed = false
	Input.parse_input_event(accept)
	await process_frame
	toggle.release_focus()
	check(toggle.text == "Stop" and not mark.disabled, "Start starts capture")
	for tick in 14:
		await physics_frame
	var captured: Dictionary = runtime.capture_status()
	check(captured.spatial_samples > 0 and captured.image_samples == 0 and captured.metric_samples > 0, "Light captures selected spatial state plus metrics, without images: " + JSON.stringify(captured))
	preset.item_selected.emit(0)
	check(runtime.capture_status().config.preset == "light", "preset cannot rewrite active segment settings")
	mark.pressed.emit()
	check(status.text.contains("Marked frame"), "marker acknowledged with agent notifications off")
	check(runtime.capture_status().last_saved_clip == null, "manual marker does not secretly save")
	var marker_key := InputEventKey.new()
	marker_key.keycode = KEY_F7
	marker_key.pressed = true
	marker_key.echo = true
	var before_markers := emitted_markers
	runtime._shortcut_input(marker_key)
	check(emitted_markers == before_markers, "repeated keys do not add markers")
	marker_key.echo = false
	var composer := ConfirmationDialog.new()
	runtime.add_child(composer)
	runtime._feedback_composer = composer
	composer.popup_centered()
	runtime._shortcut_input(marker_key)
	check(emitted_markers == before_markers, "text composition suppresses capture shortcuts")
	composer.hide()
	composer.queue_free()
	runtime._feedback_composer = null
	runtime._shortcut_input(marker_key)
	check(emitted_markers == before_markers + 1, "configured marker key works outside text composition")
	runtime._toggle_pause()
	check(paused, "pause shortcut pauses gameplay")
	runtime._toggle_pause()
	check(not paused, "pause toggles back without ending capture")
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	runtime._update_capture_controls()
	check(Input.mouse_mode == Input.MOUSE_MODE_CAPTURED, "status refresh does not seize pointer mode")
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	toggle.pressed.emit()
	check(toggle.disabled and not keep.disabled and not discard.disabled, "stopped draft requires disposition")
	var clip_path := "user://stage_recordings/" + str(runtime.capture_status().segment_id) + ".sqlite"
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("user://stage_recordings"))
	var blocker := FileAccess.open(clip_path, FileAccess.WRITE)
	blocker.store_string("reserved fixture file")
	blocker.close()
	keep.pressed.emit()
	check(runtime.capture_status().state == "review" and runtime.capture_status().last_save_error != null, "failed save retains draft")
	check(FileAccess.get_file_as_string(clip_path) == "reserved fixture file", "failure leaves existing destination unchanged")
	DirAccess.remove_absolute(ProjectSettings.globalize_path(clip_path))
	keep.pressed.emit()
	check(runtime.capture_status().state == "idle" and not resume.disabled, "retry saves and enables Continue")
	check(not status.text.contains("not saved") and not status.text.contains("Save failed"), "successful retry replaces stale error acknowledgement")
	check(not controls.find_child("CopyClipReference", true, false).disabled, "saved reference available")
	var recording_id: String = runtime.capture_status().last_saved_clip.recording_id
	resume.pressed.emit()
	check(runtime.capture_status().config.preset == "minimal", "staged preset applies to next segment")
	check(runtime.capture_status().recording_id == recording_id, "Continue groups a new segment")
	await create_timer(0.05).timeout
	toggle.pressed.emit()
	discard.pressed.emit()
	check(runtime.capture_status().state == "idle", "Discard releases only unsaved draft")
	for dimensions in [Vector2i(320, 240), Vector2i(1280, 720)]:
		root.size = dimensions
		for placement in ["top_left", "top_right", "bottom_left", "bottom_right"]:
			controls.configure("F7", placement)
			await process_frame
			await process_frame
			check(Rect2(Vector2.ZERO, Vector2(dimensions)).encloses(controls.get_global_rect()), "controls fit %s at %s: %s" % [dimensions, placement, controls.get_global_rect()])
			if placement == "bottom_right" and not OS.get_environment("CAPTURE_UI_OUTPUT").is_empty():
				await RenderingServer.frame_post_draw
				var screenshot := root.get_texture().get_image()
				check(screenshot.save_png(OS.get_environment("CAPTURE_UI_OUTPUT") + "-%dx%d.png" % [dimensions.x, dimensions.y]) == OK, "write visual QA screenshot")
	controls.refresh(runtime.capture_status())
	discard.pressed.emit()
	check(not runtime.capture_status().next_config.enabled and toggle.disabled, "Cancel disables scheduled play without deleting saved evidence")
	check(runtime.capture_status().last_saved_clip != null, "Cancel preserves kept evidence")
	controls.configure("F7", "hidden")
	var key := InputEventKey.new()
	key.keycode = KEY_F7
	key.pressed = true
	runtime._shortcut_input(key)
	check(not controls.visible and runtime._toasts.size() > 0, "hidden controls acknowledge inactive shortcut")
	OS.unset_environment("THEATRE_LAUNCH_JSON")
	finish()

func finish() -> void:
	print("CAPTURE_CONTROL_REPORT:" + JSON.stringify({"failures": failures}))
	quit(0 if failures.is_empty() else 1)
