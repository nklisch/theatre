extends SceneTree

class ClickGame extends Node:
	var clicks := 0

	func _input(event: InputEvent) -> void:
		if event is InputEventKey and event.keycode == KEY_ESCAPE and event.pressed:
			Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
		elif event is InputEventMouseButton and event.pressed:
			clicks += 1
			Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
			get_viewport().set_input_as_handled()

var failures: Array[String] = []

func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	OS.set_environment("THEATRE_LAUNCH_JSON", JSON.stringify({"options": {
		"readiness": "project", "play": {"preset": "minimal"}}}))
	root.size = Vector2i(800, 600)
	var runtime = load("res://addons/stage/runtime.gd").new()
	runtime.name = "StageRuntime"
	root.add_child(runtime)
	var game := ClickGame.new()
	root.add_child(game)
	current_scene = game
	call_deferred("_exercise", runtime, game)

func check(condition: bool, message: String) -> void:
	if not condition:
		failures.append(message)

func move_to(point: Vector2, held: bool = false) -> void:
	var motion := InputEventMouseMotion.new()
	motion.position = point
	motion.global_position = point
	motion.button_mask = MOUSE_BUTTON_MASK_LEFT if held else 0
	Input.parse_input_event(motion)
	await process_frame

func button_at(point: Vector2, pressed: bool) -> void:
	var event := InputEventMouseButton.new()
	event.position = point
	event.global_position = point
	event.button_index = MOUSE_BUTTON_LEFT
	event.pressed = pressed
	Input.parse_input_event(event)
	await process_frame

func click_at(point: Vector2) -> void:
	await move_to(point)
	for pressed in [true, false]:
		await button_at(point, pressed)

func point_of(control: Control) -> Vector2:
	return Vector2(control.get_window().position) + control.get_global_rect().get_center()

func key(code: Key) -> void:
	for pressed in [true, false]:
		var event := InputEventKey.new()
		event.keycode = code
		event.pressed = pressed
		Input.parse_input_event(event)
		await process_frame

func screenshot(suffix: String) -> void:
	var output := OS.get_environment("CAPTURE_UI_OUTPUT")
	if not output.is_empty():
		await RenderingServer.frame_post_draw
		check(root.get_texture().get_image().save_png(output + suffix + ".png") == OK, "write visual QA screenshot")

func native_activate(control: Control) -> void:
	var window := control.get_window()
	# Native OS hover is not reproduced by Input.parse_input_event. Exercise
	# native keyboard activation; real pointer dispatch is covered above through
	# the embedded window manager, including press/release on separate frames.
	control.grab_focus()
	for pressed in [true, false]:
		var event := InputEventKey.new()
		event.window_id = window.get_window_id()
		event.keycode = KEY_ENTER
		event.pressed = pressed
		Input.parse_input_event(event)
		await process_frame

func _exercise(runtime: Node, game: Node) -> void:
	await process_frame
	await process_frame
	runtime.notify_ready()
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	await process_frame
	await key(KEY_ESCAPE)
	check(Input.mouse_mode == Input.MOUSE_MODE_VISIBLE, "Escape releases the game pointer")
	var controls: Window = runtime._capture_controls
	var toggle: Button = controls.find_child("ToggleDashcam", true, false)
	var mark: Button = controls.find_child("Mark", true, false)
	var preset: OptionButton = controls.find_child("CapturePreset", true, false)
	var minimize: Button = controls.find_child("MinimizeCaptureControls", true, false)
	var handle: Control = controls.find_child("CaptureDragHandle", true, false)
	await click_at(point_of(toggle))
	check(runtime.capture_status().state == "recording", "real Start click reaches capture controls")
	check(game.clicks == 0, "capture click never reaches eager gameplay input")
	check(Input.mouse_mode == Input.MOUSE_MODE_VISIBLE, "capture click leaves released pointer visible")
	# Hold a real button across an ordinary status refresh, then repeat the click.
	var markers := [0]
	runtime.recorder.marker_added.connect(func(_frame: int, _source: String, _label: String) -> void: markers[0] += 1)
	await move_to(point_of(mark))
	await button_at(point_of(mark), true)
	await create_timer(1.1).timeout
	await button_at(point_of(mark), false)
	await click_at(point_of(mark))
	check(markers[0] == 2, "held and repeated marker clicks survive refresh")
	await key(KEY_F9)
	check(markers[0] == 3, "capture shortcut works while controls have focus")
	await click_at(point_of(preset))
	await create_timer(1.1).timeout
	check(preset.get_popup().visible, "preset popup remains open across status refresh")
	for step in 4:
		if preset.get_popup().get_focused_item() == 3:
			break
		await key(KEY_DOWN)
	await key(KEY_ENTER)
	check(runtime.capture_status().next_config.preset == "heavy", "native preset choice stages Heavy: " + str(runtime.capture_status().next_config.preset))
	check(runtime.capture_status().config.preset == "minimal", "preset choice does not rewrite active capture")
	# Move the panel using its title, and ensure status refresh does not re-dock it.
	var before := controls.position
	var start := point_of(handle)
	await move_to(start)
	await button_at(start, true)
	await move_to(start + Vector2(-180, -140), true)
	await button_at(start + Vector2(-180, -140), false)
	check(controls.position.distance_to(before + Vector2i(-180, -140)) < 3, "header drag repositions panel: " + str(controls.position))
	var moved := controls.position
	runtime._update_capture_controls()
	await process_frame
	check(controls.position == moved, "refresh preserves dragged position")
	await screenshot("-moved")
	var segment: String = runtime.capture_status().segment_id
	var samples: float = runtime.capture_status().metric_samples
	await click_at(point_of(minimize))
	await process_frame
	check(minimize.text == "Stage +" and not toggle.is_visible_in_tree() and not preset.is_visible_in_tree(), "minimize leaves only the restore button")
	check(controls.size.x < 150 and controls.size.y < 60, "minimized panel releases its former hit area")
	await create_timer(0.05).timeout
	check(runtime.capture_status().state == "recording" and runtime.capture_status().segment_id == segment and runtime.capture_status().metric_samples > samples, "minimizing leaves the same capture running")
	await screenshot("-minimized")
	paused = true
	await click_at(point_of(minimize))
	check(toggle.is_visible_in_tree() and paused, "restore works while gameplay is paused")
	paused = false
	check(controls.position == moved, "restore preserves dragged position")
	# Size changes cannot strand the panel or its restore button off screen.
	root.size = Vector2i(320, 240)
	await process_frame
	await process_frame
	check(Rect2i(Vector2i.ZERO, root.size).encloses(Rect2i(controls.position, controls.size)), "dragged panel clamps after viewport shrink")
	await click_at(point_of(minimize))
	await process_frame
	check(Rect2i(Vector2i.ZERO, root.size).encloses(Rect2i(controls.position, controls.size)), "restore button remains reachable after shrink")
	# Outside clicks still belong to the game; captured input must bypass the panel.
	var before_game: int = game.clicks
	await click_at(Vector2(2, 2))
	check(game.clicks == before_game + 1 and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED, "outside click returns input to gameplay")
	await click_at(point_of(minimize))
	check(game.clicks == before_game + 2 and minimize.text == "Stage +", "captured gameplay input cannot activate the overlay")
	await key(KEY_ESCAPE)
	await click_at(point_of(minimize))
	check(toggle.is_visible_in_tree(), "Escape then restore works repeatedly")
	await click_at(point_of(toggle))
	check(runtime.capture_status().state == "review", "real Stop click works after restore")
	await click_at(point_of(controls.find_child("DiscardCapture", true, false)))
	check(runtime.capture_status().state == "idle", "real Discard click works")
	check(Input.mouse_mode == Input.MOUSE_MODE_VISIBLE, "controls never recapture the mouse")
	# Projects can opt out of embedded subwindows. Exercise the native window's
	# own input ID and keyboard focus, without changing the project's preference.
	controls.configure("F9", "hidden")
	root.gui_embed_subwindows = false
	root.size = Vector2i(800, 600)
	await process_frame
	controls.configure("F9", "bottom_right")
	await process_frame
	await process_frame
	check(not controls.is_embedded() and not root.gui_embed_subwindows, "native subwindow setting remains unchanged")
	runtime.capture_configure({"preset": "minimal"})
	before_game = game.clicks
	controls.grab_focus()
	await native_activate(toggle)
	check(runtime.capture_status().state == "recording", "native-window keyboard Start reaches controls")
	await native_activate(minimize)
	check(minimize.text == "Stage +" and runtime.capture_status().state == "recording", "native minimize preserves recording")
	await native_activate(minimize)
	await native_activate(toggle)
	check(runtime.capture_status().state == "review", "native restore and Stop work")
	await native_activate(controls.find_child("DiscardCapture", true, false))
	check(runtime.capture_status().state == "idle" and game.clicks == before_game, "native keyboard control leaves gameplay unchanged")
	controls.hide()
	OS.unset_environment("THEATRE_LAUNCH_JSON")
	print("CAPTURE_POINTER_REPORT:" + JSON.stringify({"failures": failures}))
	quit(0 if failures.is_empty() else 1)
