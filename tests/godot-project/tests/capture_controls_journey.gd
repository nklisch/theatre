extends SceneTree

var failures: Array[String] = []

func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	ProjectSettings.set_setting("theatre/stage/dashcam/enabled", false)
	ProjectSettings.set_setting("theatre/stage/display/show_agent_notifications", false)
	ProjectSettings.set_setting("theatre/stage/display/capture_controls", "bottom_right")
	ProjectSettings.set_setting("theatre/stage/shortcuts/marker_key", "F7")
	root.size = Vector2i(320, 240)
	var scene := Node2D.new()
	scene.name = "CaptureJourney"
	scene.scene_file_path = "res://before_transition.tscn"
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
	var recorder = runtime.recorder
	if not check(recorder != null, "recorder initialized"):
		finish()
		return
	var controls: Control = runtime.get("_capture_controls")
	var mark: Button = controls.find_child("Mark", true, false)
	var toggle: Button = controls.find_child("ToggleDashcam", true, false)
	var save: Button = controls.find_child("SaveNow", true, false)
	var status: Label = controls.find_child("CaptureStatus", true, false)
	check(mark.text.contains("F7"), "configured marker shortcut is displayed")
	check(mark.disabled and save.disabled and toggle.text == "Start", "stopped controls are truthful")
	var key := InputEventKey.new()
	key.keycode = KEY_F7
	key.pressed = true
	runtime._shortcut_input(key)
	check(status.text.contains("Start recording"), "disabled shortcut explains why no marker was saved")
	for dimensions in [Vector2i(320,240), Vector2i(1280,720)]:
		root.size = dimensions
		for placement in ["top_left", "top_right", "bottom_left", "bottom_right"]:
			controls.configure("F7", placement)
			await process_frame
			await process_frame
			var bounds := Rect2(Vector2.ZERO, Vector2(dimensions))
			check(bounds.encloses(controls.get_global_rect()), "controls fit %s at %s: %s" % [dimensions, placement, controls.get_global_rect()])
	if not OS.get_environment("CAPTURE_SCREENSHOT_PATH").is_empty():
		await RenderingServer.frame_post_draw
		root.get_texture().get_image().save_png(OS.get_environment("CAPTURE_SCREENSHOT_PATH"))
	controls.configure("F7", "hidden")
	check(not controls.visible, "hidden placement removes the launcher")
	runtime._shortcut_input(key)
	check(runtime.get("_toasts").size() > 0, "hidden shortcut acknowledgement survives disabled agent notifications")
	controls.configure("F7", "bottom_right")
	var patch: Dictionary = JSON.parse_string(recorder.apply_dashcam_config(JSON.stringify({
		"post_window_deliberate_sec": 1, "min_after_sec": 0,
		"anomaly_enabled": false, "screenshot_enabled": false
	})))
	check(patch.get("result") == "ok", "test recording settings applied")
	toggle.pressed.emit()
	check(recorder.is_dashcam_active() and toggle.text == "Stop", "Start begins recording and updates controls")
	await create_timer(0.15).timeout
	runtime._update_capture_controls()
	mark.pressed.emit()
	check(status.text.contains("Marked frame"), "human marker is immediately acknowledged")
	check(recorder.get_dashcam_state() == "post_capture", "Mark retains its post-window rather than immediately saving")
	await create_timer(1.2).timeout
	var capture: Dictionary = JSON.parse_string(recorder.get_dashcam_status_json())
	check(capture.get("last_saved_clip") is Dictionary, "marked post-window saved a clip")
	if capture.get("last_saved_clip") is Dictionary:
		var first_id: String = capture.last_saved_clip.clip_id
		check(controls.find_child("CopyClipReference", true, false).disabled == false, "saved reference is available")
		mark.pressed.emit()
		var previous_scene := current_scene
		var after := Node2D.new()
		after.name = "AfterTransition"
		after.scene_file_path = "res://after_transition.tscn"
		root.add_child(after)
		current_scene = after
		previous_scene.queue_free()
		await physics_frame
		await physics_frame
		save.pressed.emit()
		capture = JSON.parse_string(recorder.get_dashcam_status_json())
		check(capture.last_saved_clip.clip_id != first_id, "Save now closes a new pending clip immediately")
		check(capture.last_saved_clip.get("scene_at_save") == "res://after_transition.tscn", "scene provenance is explicitly the scene at save")
		check(not capture.last_saved_clip.has("scene_path"), "mixed-scene clip does not claim an unqualified scene")
	mark.pressed.emit()
	toggle.pressed.emit()
	capture = JSON.parse_string(recorder.get_dashcam_status_json())
	check(capture.state == "disabled", "Stop disables recording")
	check(status.text.contains("available capture saved"), "Stop acknowledges saving a shortened pending window")
	runtime._apply_capture_preset("lightweight")
	check(not recorder.is_dashcam_active(), "preset choice does not enable recording")
	# A real filesystem failure must not publish a new successful-save identity.
	var hint := FileAccess.open("res://.stage/clip_storage_path", FileAccess.READ)
	var storage := hint.get_as_text().strip_edges().trim_suffix("/")
	hint.close()
	var prior_clip: String = capture.last_saved_clip.clip_id
	check(DirAccess.rename_absolute(storage, storage + "_retained") == OK, "isolate saved files for failure injection")
	var blocker := FileAccess.open(storage, FileAccess.WRITE)
	blocker.store_string("deliberate storage obstruction")
	blocker.close()
	runtime._toggle_capture()
	await create_timer(0.15).timeout
	runtime._save_capture_now()
	capture = JSON.parse_string(recorder.get_dashcam_status_json())
	check(capture.last_save_error != null, "failed persistence is exposed")
	check(capture.last_saved_clip.clip_id == prior_clip, "failed save does not replace the last successful reference")
	check(status.text.contains("not saved"), "human sees failure rather than a saved acknowledgement")
	DirAccess.remove_absolute(storage)
	DirAccess.rename_absolute(storage + "_retained", storage)
	runtime._save_capture_now()
	capture = JSON.parse_string(recorder.get_dashcam_status_json())
	check(capture.last_save_error == null and capture.last_saved_clip.clip_id != prior_clip, "explicit save succeeds after storage is restored")
	finish()

func finish() -> void:
	print("CAPTURE_CONTROL_REPORT:" + JSON.stringify({"failures": failures}))
	quit(0 if failures.is_empty() else 1)
