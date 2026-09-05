extends SceneTree

func _initialize() -> void:
	call_deferred("run")

func run() -> void:
	assert(GDExtensionManager.load_extension("res://addons/stage/stage.gdextension") == GDExtensionManager.LOAD_STATUS_OK)
	ProjectSettings.set_setting("theatre/stage/dashcam/enabled", false)
	var scene := Node2D.new()
	scene.name = "RuntimeFeedbackScene"
	root.add_child(scene)
	current_scene = scene
	var runtime = load("res://addons/stage/runtime.gd").new()
	root.add_child(runtime)
	await process_frame
	await process_frame
	assert(runtime.tcp_server != null)
	var run_id: String = runtime.tcp_server.get_run_id()
	assert(not run_id.is_empty())
	var recorder_state: String = runtime.recorder.get_dashcam_state()
	runtime.share_feedback()
	assert(not paused)
	assert(runtime._feedback_composer._composition.item.run_id == run_id)
	assert(runtime._feedback_composer._composition.item.capture.status == "unavailable")
	runtime._feedback_composer._note.text = "Runtime entrypoint without agent connection"
	runtime._feedback_composer._queue()
	assert(runtime.recorder.get_dashcam_state() == recorder_state)
	assert(runtime.has_method("marker"))
	await process_frame
	# Sharing must also leave an active continuous recorder and gameplay alone.
	runtime.recorder.set_dashcam_enabled(true)
	await physics_frame
	assert(runtime.recorder.is_dashcam_active())
	var frame_before := Engine.get_physics_frames()
	runtime.marker("feedback compatibility check", "silent")
	runtime.share_feedback()
	runtime._feedback_composer._note.text = "Sharing while recorder stays active"
	runtime._feedback_composer._queue()
	await physics_frame
	assert(Engine.get_physics_frames() > frame_before)
	assert(not paused and runtime.recorder.is_dashcam_active())
	print("FEEDBACK_STAGE_OK")
	quit()
