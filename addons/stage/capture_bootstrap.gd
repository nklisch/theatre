extends Node
## Do not preload the target: startup capture must precede its resource load.


func _ready() -> void:
	call_deferred("_load_target")


func _load_target() -> void:
	var runtime := get_node_or_null("/root/StageRuntime")
	if runtime == null or runtime.target_scene.is_empty():
		push_error("[Stage] Startup bootstrap requires StageRuntime and an explicit target scene.")
		return
	# Retain a boundary sample before any synchronous loading blocks the engine.
	await get_tree().process_frame
	var target: String = runtime.target_scene
	if not target.begins_with("res://") or target == scene_file_path:
		push_error("[Stage] Invalid bootstrap target.")
		runtime.capture_stop("invalid_startup_target")
		return
	var error := get_tree().change_scene_to_file(target)
	if error != OK:
		push_error("[Stage] Target scene failed to load: " + error_string(error))
		runtime.capture_stop("startup_load_failed")
