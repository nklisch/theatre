extends SceneTree

var failures: Array[String] = []
var provider_calls := 0
var metadata_calls := 0


func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	call_deferred("_exercise")


func check(condition: bool, message: String) -> void:
	if not condition:
		failures.append(message)


func provider() -> Dictionary:
	provider_calls += 1
	return {"work_units": provider_calls}


func oversized_provider() -> Dictionary:
	provider_calls += 1
	return {"payload": "x".repeat(2 * 1024 * 1024)}


func metadata_provider() -> Dictionary:
	metadata_calls += 1
	return {"scenario": "self-contained fixture"}


func ticks(count: int) -> void:
	for index in count:
		await process_frame


func _exercise() -> void:
	var script = load("res://addons/stage/runtime.gd")
	OS.unset_environment("THEATRE_LAUNCH_JSON")
	var off = script.new()
	root.add_child(off)
	await ticks(3)
	check(off.get_child_count() == 0, "off creates no collector, recorder, listener or controls")
	check(not off.is_processing() and not off.is_physics_processing(), "off has no periodic processing")
	check(off._runtime_logger == null, "off has no runtime logger")
	off.notify_ready()
	off.marker("safe while off")
	off.free()
	OS.set_environment("THEATRE_LAUNCH_JSON", JSON.stringify({"target_scene": "res://fixture.tscn", "options": {
		"startup": {"preset": "minimal"}, "play": {"preset": "minimal", "max_records": 8}, "readiness": "project"}}))
	var runtime = script.new()
	root.add_child(runtime)
	runtime.register_metric_provider("fixture", provider)
	runtime.register_metadata_provider("fixture", metadata_provider)
	await ticks(4)
	check(runtime.collector == null and runtime.tcp_server == null and runtime._runtime_logger == null, "metrics only has no spatial or live services")
	check(provider_calls > 0, "provider sampled while startup active")
	check(not runtime.capture_status().ready, "does not infer asynchronous readiness")
	runtime.notify_ready()
	var ready: Dictionary = runtime.capture_status()
	check(ready.state == "idle" and ready.last_saved_clip is Dictionary, "startup saved at readiness")
	var startup_id: String = ready.last_saved_clip.clip_id
	var recording_id: String = ready.last_saved_clip.recording_id
	var calls_at_ready := provider_calls
	await ticks(5)
	runtime.notify_ready()
	check(provider_calls == calls_at_ready, "human wait and duplicate ready never sample or start play")
	check(metadata_calls == 0, "registering metadata does not sample during human wait")
	var continued: Dictionary = runtime.capture_start(true)
	check(metadata_calls == 1 and continued.context.project_metadata.fixture.scenario == "self-contained fixture", "explicit Start snapshots provider metadata")
	check(continued.get("recording_id") == recording_id, "continue retains recording identity")
	await ticks(12)
	var bounded: Dictionary = runtime.capture_status()
	check(bounded.state == "review" and bounded.metric_samples <= 8, "record limit stops without rolling session evidence")
	var calls_at_stop := provider_calls
	await ticks(3)
	check(provider_calls == calls_at_stop, "provider stops at automatic limit")
	check(runtime.capture_start().has("error"), "cannot overwrite an unkept draft")
	check(metadata_calls == 1, "rejected Start does not invoke metadata providers")
	var kept: Dictionary = runtime.capture_keep("fixture note")
	check(kept.state == "idle" and kept.last_saved_clip.clip_id != startup_id, "play saves a separate immutable segment")
	check(kept.last_saved_clip.recording_id == recording_id, "saved segment preserves grouping")
	check(kept.last_saved_clip.capture.context.project_metadata.fixture.scenario == "self-contained fixture", "kept evidence preserves project metadata")
	var fresh: Dictionary = runtime.capture_start()
	check(fresh.recording_id != recording_id, "Start new has independent identity")
	await ticks(2)
	runtime.capture_stop()
	runtime.capture_discard()
	check(runtime.capture_status().state == "idle", "discard releases the draft")
	var next: Dictionary = runtime.capture_configure({"retention": "rolling", "payload_mib": 1, "max_records": 100})
	check(next.next_config.preset == "minimal", "partial next settings preserve per-run preset")
	check(next.launch.sources["play.payload_mib"] == "runtime", "staged settings report provenance")
	runtime.register_metric_provider("oversized", oversized_provider)
	runtime.capture_start()
	await ticks(3)
	var oversized: Dictionary = runtime.capture_status()
	check(oversized.state == "review" and oversized.stop_reason == "oversized_sample", "oversized rolling sample stops rather than sampling forever")
	calls_at_stop = provider_calls
	await ticks(3)
	check(provider_calls == calls_at_stop, "oversized stop disables providers")
	runtime.unregister_metric_provider("oversized")
	runtime.capture_discard()
	runtime.capture_configure({"save": "on_trigger"})
	runtime.capture_start(true)
	await ticks(2)
	runtime.marker("shortened post-window")
	check(runtime.capture_status().pending_post_window, "explicit trigger starts bounded post-window")
	var shortened: Dictionary = runtime.capture_stop()
	check(shortened.state == "idle" and shortened.last_saved_clip.capture.post_window_shortened, "Stop saves available pending window with shortened coverage")
	runtime.capture_configure({"save": "manual", "max_records": 1})
	runtime.capture_start()
	runtime.marker("first system marker")
	runtime.marker("suppressed oversized marker " + "x".repeat(1024))
	# Before another process callback, the suppressed marker must not evict the first.
	var marked: Dictionary = runtime.capture_stop()
	check(marked.state == "review", "system marker burst leaves a reviewable draft")
	var marked_clip: Dictionary = runtime.capture_keep()
	var markers: Array = runtime.recorder.get_recording_markers("user://stage_recordings", marked_clip.last_saved_clip.clip_id)
	check(markers.size() == 1 and markers[0].label == "first system marker", "rate-limited marker never replaces retained evidence")
	runtime.free()
	OS.set_environment("THEATRE_LAUNCH_JSON", JSON.stringify({"target_scene": "res://fixture.tscn", "options": {
		"startup": {"preset": "minimal", "max_records": 2}, "play": {"preset": "minimal"}}}))
	var missing_ready = script.new()
	root.add_child(missing_ready)
	await ticks(8)
	var missing: Dictionary = missing_ready.capture_status()
	check(not missing.ready and missing.state == "idle", "missing readiness stops at bound without inventing readiness")
	check(missing.last_saved_clip.capture.stop_reason == "record_limit", "startup limit saves available evidence")
	check(missing.last_saved_clip.capture.context.target_scene == "res://fixture.tscn", "saved evidence retains requested target")
	missing_ready.free()
	var scene := Node2D.new()
	root.add_child(scene)
	current_scene = scene
	OS.set_environment("THEATRE_LAUNCH_JSON", JSON.stringify({"target_scene": "res://fixture.tscn", "options": {
		"operator": "agent", "startup": {"preset": "minimal"}, "play": {"preset": "light", "scope": ["."]}}}))
	var agent = script.new()
	root.add_child(agent)
	await ticks(3)
	check(agent.collector == null, "agent minimal startup does not create spatial collector")
	agent.notify_ready()
	await ticks(14)
	var playing: Dictionary = agent.capture_status()
	check(playing.state == "recording" and playing.phase == "play" and playing.spatial_samples > 0, "agent automatically switches metrics startup to scoped spatial play")
	check(playing.recording_id == playing.last_saved_clip.recording_id, "agent phases retain recording identity")
	check(playing.last_saved_clip.capture.config.preset == "minimal", "richer play does not rewrite saved startup settings")
	agent.capture_stop()
	agent.capture_discard()
	agent.free()
	scene.free()
	OS.unset_environment("THEATRE_LAUNCH_JSON")
	for failure in failures:
		push_error(failure)
	print("CAPTURE_LIFECYCLE_JOURNEY ", "PASS" if failures.is_empty() else "FAIL")
	quit(0 if failures.is_empty() else 1)
