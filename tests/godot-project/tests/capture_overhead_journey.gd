extends SceneTree

var bodies: Array[Node2D] = []
var physics_ms: Array[float] = []
var render_intervals_ms: Array[float] = []
var measuring := false
var last_render_usec := 0
var baseline_total_usec := 0
var baseline_samples := 0
var profile := ""
var peak_memory_bytes := 0

func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	profile = OS.get_environment("CAPTURE_PROFILE")
	var options := {}
	if profile not in ["off", "baseline"]:
		options = {"play": {"preset": profile, "scope": ["group:measured"] if profile != "heavy" else []}, "readiness": "project"}
	OS.set_environment("THEATRE_LAUNCH_JSON", JSON.stringify({"options": options}))
	ProjectSettings.set_setting("theatre/stage/display/capture_controls", "hidden")
	ProjectSettings.set_setting("theatre/stage/display/show_agent_notifications", false)
	root.size = Vector2i(1280, 720)
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
		if index < 8:
			body.add_to_group("measured")
		bodies.append(body)
	var runtime = load("res://addons/stage/runtime.gd").new()
	runtime.name = "StageRuntime"
	root.add_child(runtime)
	physics_frame.connect(_animate)
	process_frame.connect(_render_tick)
	call_deferred("_measure", runtime)

func _metric_values() -> Dictionary:
	return {"body_count": bodies.size(), "physics_frame": Engine.get_physics_frames()}

func _render_tick() -> void:
	var now := Time.get_ticks_usec()
	if measuring and last_render_usec > 0:
		render_intervals_ms.append(float(now - last_render_usec) / 1000.0)
		peak_memory_bytes = maxi(peak_memory_bytes, int(Performance.get_monitor(Performance.MEMORY_STATIC)))
	last_render_usec = now
	if profile == "baseline":
		var started := Time.get_ticks_usec()
		_metric_values()
		var elapsed := Time.get_ticks_usec() - started
		if measuring:
			baseline_total_usec += elapsed
			baseline_samples += 1

func _animate() -> void:
	var phase := float(Engine.get_physics_frames()) / 60.0
	for index in bodies.size():
		bodies[index].position = Vector2(80 + (index % 8) * 150, 65 + (index / 8) * 80) + Vector2(sin(phase + index), cos(phase + index)) * 15
	if measuring:
		physics_ms.append(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0)

func _measure(runtime: Node) -> void:
	await process_frame
	runtime.register_metric_provider("fixture", _metric_values)
	runtime.notify_ready()
	if profile not in ["off", "baseline"]:
		var started_capture: Dictionary = runtime.capture_start()
		if started_capture.has("error"):
			push_error("Benchmark capture could not start: " + str(started_capture))
			quit(1)
			return
	var warmup := Time.get_ticks_usec()
	while Time.get_ticks_usec() - warmup < 1000000:
		await process_frame
	var started := Time.get_ticks_usec()
	var first_frame := Engine.get_physics_frames()
	var memory_start := int(Performance.get_monitor(Performance.MEMORY_STATIC))
	peak_memory_bytes = memory_start
	measuring = true
	while Time.get_ticks_usec() - started < 5000000:
		await process_frame
	measuring = false
	var elapsed := float(Time.get_ticks_usec() - started) / 1000000.0
	physics_ms.sort()
	render_intervals_ms.sort()
	var capture: Dictionary = runtime.capture_status()
	var memory_end := int(Performance.get_monitor(Performance.MEMORY_STATIC))
	var finalization_usec := 0
	var persistence_usec := 0
	var persisted_bytes := 0
	if runtime.recorder != null:
		var before_stop := Time.get_ticks_usec()
		runtime.capture_stop()
		finalization_usec = Time.get_ticks_usec() - before_stop
		var before_keep := Time.get_ticks_usec()
		var kept: Dictionary = runtime.capture_keep("benchmark fixture")
		persistence_usec = Time.get_ticks_usec() - before_keep
		if kept.has("error"):
			push_error("Benchmark save failed: " + str(kept))
			quit(1)
			return
		var saved := FileAccess.open("user://stage_recordings/" + str(kept.last_saved_clip.clip_id) + ".sqlite", FileAccess.READ)
		if saved != null:
			persisted_bytes = saved.get_length()
			saved.close()
	print("CAPTURE_BENCHMARK:" + JSON.stringify({
		"profile": profile, "scene": "64 moving Polygon2D nodes; Light/Standard scope eight group members; 1280x720",
		"godot_version": Engine.get_version_info().get("string", "unknown"),
		"rendering_method": RenderingServer.get_current_rendering_method(),
		"physics_ticks_per_second": Engine.physics_ticks_per_second,
		"measurement_seconds": elapsed, "physics_ticks": Engine.get_physics_frames() - first_frame,
		"physics_ms_median": physics_ms[physics_ms.size() / 2],
		"physics_ms_p95": physics_ms[int(physics_ms.size() * 0.95)],
		"render_interval_ms_median": render_intervals_ms[render_intervals_ms.size() / 2],
		"render_interval_ms_p95": render_intervals_ms[int(render_intervals_ms.size() * 0.95)],
		"baseline_callback_total_usec": baseline_total_usec, "baseline_samples": baseline_samples,
		"finalization_usec": finalization_usec, "persistence_usec": persistence_usec,
		"persisted_bytes": persisted_bytes, "memory_static_start_bytes": memory_start,
		"memory_static_end_bytes": memory_end, "memory_static_peak_bytes": peak_memory_bytes,
		"memory_static_after_keep_bytes": int(Performance.get_monitor(Performance.MEMORY_STATIC)),
		"collector_created": runtime.collector != null, "listener_created": runtime.tcp_server != null,
		"status": capture,
		"limitations": ["Local fixture/build measurement, not a universal performance guarantee.", "Render intervals include pacing; physics monitor excludes some observer work.", "Native probe includes warmup; baseline is provider/timing only, not recording.", "Godot static memory excludes some native allocations and is not process RSS."]
	}))
	OS.unset_environment("THEATRE_LAUNCH_JSON")
	quit(0)
