extends SceneTree

var bodies: Array[Node2D] = []
var physics_ms: Array[float] = []
var measuring := false

func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	ProjectSettings.set_setting("theatre/stage/dashcam/enabled", false)
	ProjectSettings.set_setting("theatre/stage/display/capture_controls", "hidden")
	ProjectSettings.set_setting("theatre/stage/display/show_agent_notifications", false)
	root.size = Vector2i(1280,720)
	var scene := Node2D.new()
	scene.name = "MovingShapes"
	root.add_child(scene)
	current_scene = scene
	for index in 64:
		var body := Polygon2D.new()
		body.name = "Body%d" % index
		body.polygon = PackedVector2Array([Vector2(-16,-16),Vector2(16,-16),Vector2(16,16),Vector2(-16,16)])
		body.color = Color.from_hsv(float(index) / 64.0, 0.8, 0.9)
		scene.add_child(body)
		bodies.append(body)
	var runtime = load("res://addons/stage/runtime.gd").new()
	runtime.name = "StageRuntime"
	root.add_child(runtime)
	physics_frame.connect(_animate)
	call_deferred("_measure", runtime)

func _animate() -> void:
	var phase := float(Engine.get_physics_frames()) / 60.0
	for index in bodies.size():
		bodies[index].position = Vector2(80 + (index % 8) * 150, 65 + (index / 8) * 80) + Vector2(sin(phase + index), cos(phase + index)) * 15
	if measuring:
		physics_ms.append(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0)

func _measure(runtime: Node) -> void:
	await process_frame
	var profile := OS.get_environment("CAPTURE_PROFILE")
	var patch := {"enabled": profile != "disabled", "anomaly_enabled": false}
	if profile != "disabled":
		patch["preset"] = "lightweight" if profile in ["lightweight_no_images", "spatial_only"] else profile
	if profile == "lightweight_no_images":
		# Isolate image capture without changing Lightweight's spatial sampling.
		patch["screenshot_enabled"] = false
	var applied: Dictionary = JSON.parse_string(runtime.recorder.apply_dashcam_config(JSON.stringify(patch)))
	if applied.get("result") == "ok" and profile == "spatial_only":
		# Select after Lightweight to prove that Spatial only preserves its cadence.
		applied = JSON.parse_string(runtime.recorder.apply_dashcam_config(JSON.stringify({"preset": "spatial_only"})))
	if applied.get("result") != "ok":
		push_error("Benchmark configuration failed: %s" % applied)
		quit(1)
		return
	await create_timer(1.0).timeout
	var start := Time.get_ticks_usec()
	var first_frame := Engine.get_physics_frames()
	measuring = true
	await create_timer(5.0).timeout
	measuring = false
	var elapsed := float(Time.get_ticks_usec() - start) / 1000000.0
	physics_ms.sort()
	var capture: Dictionary = JSON.parse_string(runtime.recorder.get_dashcam_status_json())
	print("CAPTURE_BENCHMARK:" + JSON.stringify({
		"profile":profile, "scene":"64 moving Polygon2D nodes, 1280x720, compatibility renderer",
		"godot_version":Engine.get_version_info().get("string", "unknown"),
		"rendering_method":RenderingServer.get_current_rendering_method(),
		"physics_ticks_per_second":Engine.physics_ticks_per_second,
		"warmup_seconds":1.0,
		"physics_monitor":"Performance.TIME_PHYSICS_PROCESS; sampled during the measurement window",
		"pacing_window":"Recorder's latest 600 tick intervals; includes warmup",
		"measurement_seconds":elapsed, "physics_ticks":Engine.get_physics_frames() - first_frame,
		"physics_ms_median":physics_ms[physics_ms.size() / 2],
		"physics_ms_p95":physics_ms[int(physics_ms.size() * 0.95)],
		"status":capture
	}))
	quit(0)
