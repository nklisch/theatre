extends SceneTree

var recorder: Node
var failures: Array[String] = []
var reports: Array[Dictionary] = []

func check(condition: bool, message: String) -> void:
	if not condition:
		failures.append(message)

func status() -> Dictionary:
	return JSON.parse_string(recorder.get_dashcam_status_json())

func configure(patch: Dictionary) -> void:
	var response: Dictionary = JSON.parse_string(recorder.apply_dashcam_config(JSON.stringify(patch)))
	check(not response.has("error"), "Configuration rejected: " + JSON.stringify(response))

func ticks(count: int) -> void:
	for tick in range(count):
		await physics_frame

func wait_pending() -> void:
	for tick in range(30):
		if status().screenshot_capture.pending:
			return
		await physics_frame
	check(false, "Expected pending GPU transfer")

func save_report(phase: String, width: int, height: int) -> void:
	var clip_id: String = recorder.flush_dashcam_clip("native readback " + phase)
	check(not clip_id.is_empty(), "Saving " + phase + " failed")
	reports.append({"phase":phase,"width":width,"height":height,"status":status()})

func _initialize() -> void:
	call_deferred("run")

func run() -> void:
	load("res://stage.gdextension")
	root.size = Vector2i(640, 360)
	var scene := Node2D.new()
	root.add_child(scene)
	current_scene = scene
	var upper := ColorRect.new()
	upper.color = Color.RED
	upper.size = Vector2(640, 180)
	scene.add_child(upper)
	var lower := ColorRect.new()
	lower.color = Color.BLUE
	lower.position.y = 180
	lower.size = Vector2(640, 180)
	scene.add_child(lower)
	var collector = ClassDB.instantiate("StageCollector")
	root.add_child(collector)
	recorder = ClassDB.instantiate("StageRecorder")
	recorder.set_dashcam_enabled(false)
	root.add_child(recorder)
	recorder.set_collector(collector)
	var mode := OS.get_environment("READBACK_MODE")
	configure({"enabled":true,"screenshot_interval_frames":1,"screenshot_max_dimension":160,"screenshot_encode_queue":1,"capture_interval":6,"screenshot_readback":"synchronous" if mode == "synchronous" else "auto"})
	await ticks(60)
	var initial := status()
	if mode == "headless":
		check(initial.buffer_frames > 0, "Headless spatial capture missing")
		check(initial.screenshot_capture.backend == "unavailable" and initial.screenshot_capture.reason == "headless", "Headless capability must be unavailable")
		check(initial.screenshot_buffer_count == 0 and initial.capture_probe.dispatched == 0, "Headless capture attempted pixels")
		configure({"screenshot_readback":"synchronous"})
		await ticks(6)
		check(status().screenshot_capture.backend == "unavailable", "Explicit sync cannot manufacture a headless viewport")
	elif mode == "forward_plus":
		check(RenderingServer.get_current_rendering_method() == "forward_plus", "Journey must actually use Forward+")
		check(initial.buffer_frames > 0, "Forward+ auto lost spatial capture")
		check(not initial.screenshot_capture.available and initial.screenshot_capture.backend == "unavailable", "Forward+ auto must report visual unavailability")
		check(initial.screenshot_capture.reason == "Native asynchronous capture requires Compatibility/OpenGL; synchronous recovery is explicit", "Forward+ auto must explain explicit recovery")
		check(initial.screenshot_buffer_count == 0 and initial.capture_probe.dispatched == 0, "Forward+ auto attempted pixels instead of remaining unavailable")
		save_report("auto_unavailable", 0, 0)
		configure({"screenshot_readback":"synchronous"})
		await ticks(60)
		var recovered := status()
		check(RenderingServer.get_current_rendering_method() == "forward_plus", "Recovery must not change the renderer")
		check(recovered.screenshot_capture.available and recovered.screenshot_capture.backend == "synchronous", "Forward+ explicit synchronous recovery unavailable")
		check(recovered.screenshot_buffer_count > 0 and recovered.capture_probe.dispatched > 0, "Forward+ explicit recovery produced no images")
		check(recovered.buffer_frames > initial.buffer_frames, "Forward+ recovery interrupted spatial recording")
		# The Rust side decodes this saved JPEG and checks dimensions and colors.
		save_report("synchronous_recovery", 160, 90)
	else:
		check(initial.screenshot_buffer_count > 0, "Images missing: " + JSON.stringify(initial.screenshot_capture))
		check(initial.capture_probe.encode_depth_max <= 1, "Outstanding admission exceeded configured limit")
		if mode != "synchronous":
			check(initial.screenshot_capture.backend == "opengl_async", "Native backend not verified")
			check(initial.capture_probe.last_completion_frame > initial.capture_probe.last_request_frame, "Image did not complete on a later frame")
			check(initial.capture_probe.dropped_queue_full > 0, "Single-slot admission did not reject pending samples")
			await wait_pending()
		else:
			check(initial.screenshot_capture.backend == "synchronous", "Explicit recovery not selected")
		save_report("initial", 160, 90)
		if mode != "synchronous":
			# A loading/main-thread stall ages a valid GPU request. Wall time is
			# not a capability failure; resume polling rather than disabling Auto.
			OS.delay_msec(2100)
			await ticks(6)
			check(status().screenshot_capture.backend == "opengl_async", "Loading stall permanently disabled native capture")
			await wait_pending()
			# Save in the same tick as invalidation: native ownership remains
			# pending, but its lost image must be accounted for only once.
			configure({"preset":"spatial_only"})
			check(status().screenshot_capture.pending, "Spatial-only must retain pending native ownership")
			save_report("invalidated_pending", 160, 90)
			configure({"screenshot_enabled":true})
			await ticks(6)
			await wait_pending()
		# Stop with a transfer outstanding. It must not publish into the stopped
		# buffer or a restarted generation, and Save now must not drain it.
		var before_stop := Time.get_ticks_usec()
		recorder.set_dashcam_enabled(false)
		check(Time.get_ticks_usec() - before_stop < 100000, "Ordinary Stop waited for capture work")
		upper.color = Color.GREEN
		lower.color = Color.GREEN
		configure({"enabled":true,"preset":"spatial_only"})
		await ticks(10)
		check(status().screenshot_buffer_count == 0, "Old generation entered restarted buffer")
		check(status().buffer_frames > 0, "Spatial-only did not preserve recording")
		check(status().screenshot_capture.backend == "disabled", "Spatial-only capability should be disabled")
		var triggers_before: float = status().anomaly.triggers_total
		configure({"screenshot_enabled":true,"anomaly_min_proportion":0.001,"anomaly_relative_factor":1.0,"anomaly_sustained_frames":1})
		await ticks(30)
		check(status().anomaly.triggers_total == triggers_before, "Cross-generation pixels triggered an anomaly")
		check(status().anomaly.last_proportion == 0, "Cross-generation comparison continuity leaked")
		# Resize/configuration invalidate publication, not the native texture's
		# ownership. The next sample must use a newly sized drawable.
		if mode != "synchronous":
			await wait_pending()
		root.size = Vector2i(800, 400)
		upper.color = Color.RED
		upper.size = Vector2(800, 200)
		lower.color = Color.BLUE
		lower.position.y = 200
		lower.size = Vector2(800, 200)
		configure({"screenshot_max_dimension":80,"anomaly_enabled":false,"screenshot_encode_queue":2})
		await ticks(36)
		save_report("resized", 80, 40)
		if mode != "synchronous":
			await wait_pending()
			configure({"screenshot_readback":"synchronous"})
			await ticks(12)
			check(status().screenshot_capture.backend == "synchronous", "Pending native-to-sync transition failed")
			configure({"screenshot_readback":"auto"})
			await ticks(12)
			check(status().screenshot_capture.backend == "opengl_async", "Synchronous-to-native transition failed")
			await wait_pending()
	# Destruction with pending native ownership must retire in render context.
	recorder.queue_free()
	await process_frame
	print("NATIVE_READBACK_REPORT:" + JSON.stringify({"failures":failures,"reports":reports,"initial":initial,"storage_path":ProjectSettings.globalize_path("user://stage_recordings/")}))
	quit(0 if failures.is_empty() else 1)
