@tool

const Dispatcher = preload("res://addons/director/ops/dispatcher.gd")
const EditorRun = preload("res://addons/director/editor_run.gd")
const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")

static func dispatch(operation: String, params: Dictionary, manager: EditorUndoRedoManager = null) -> Dictionary:
	if operation == "editor_status":
		return _editor_status()
	if operation == "editor_run":
		return EditorRun.dispatch(params)
	if operation == "batch":
		return Dispatcher.MetaOps.op_batch(params, dispatch.bind(manager))
	# Replacing an open scene would silently destroy the human's unsaved work.
	var destination: String = params.get("dest_path", params.get("save_path", params.get("output_path", "")))
	if operation in ["material_create", "style_box_create", "visual_shader_create", "animation_create"]:
		destination = params.get("resource_path", "")
	if operation == "scene_create":
		destination = params.get("scene_path", "")
	if destination != "" and find_open_root(destination) != null:
		return OpsUtil._error("Destination is open in the editor; use scene mutations or a new destination", operation, params)
	var root: Node = null
	if operation in SceneEdit.OPERATIONS:
		root = find_open_root(params.get("scene_path", ""))
	var result: Dictionary = Dispatcher.dispatch(operation, params, root, manager)
	if not result.get("persistence", {}).get("saved_paths", []).is_empty():
		EditorInterface.get_resource_filesystem().scan()
	return result

static func find_open_root(scene_path: String) -> Node:
	var full_path := ("res://" + scene_path.trim_prefix("res://")).simplify_path()
	if not full_path in EditorInterface.get_open_scenes():
		return null
	for root in EditorInterface.get_open_scene_roots():
		if root.scene_file_path == full_path:
			return root
	return null

static func _editor_status() -> Dictionary:
	## Return a live snapshot of the Godot editor's state.
	var open_scenes := EditorInterface.get_open_scenes()
	var active_root := EditorInterface.get_edited_scene_root()
	var active_scene := ""
	if active_root != null:
		active_scene = active_root.scene_file_path.trim_prefix("res://")

	var playing := EditorInterface.is_playing_scene()

	# Read autoloads from project.godot
	var autoloads: Dictionary = {}
	var cfg := ConfigFile.new()
	if cfg.load("res://project.godot") == OK:
		if cfg.has_section("autoload"):
			for key in cfg.get_section_keys("autoload"):
				var value: String = str(cfg.get_value("autoload", key, ""))
				autoloads[key] = value.trim_prefix("*").trim_prefix("res://")

	# Clean up open_scenes paths
	var cleaned_scenes: Array[String] = []
	for s in open_scenes:
		cleaned_scenes.append(s.trim_prefix("res://"))

	# Read recent log
	var recent_log: Array[String] = _read_recent_log()

	return {"success": true, "data": {
		"editor_connected": true,
		"project_path": ProjectSettings.globalize_path("res://"),
		"process_id": OS.get_process_id(),
		"active_scene": active_scene,
		"open_scenes": cleaned_scenes,
		"game_running": playing,
		"autoloads": autoloads,
		"recent_log": recent_log,
	}}


static func _read_recent_log() -> Array[String]:
	## Read the last 50 non-empty lines from godot.log.
	var log_path := OS.get_user_data_dir() + "/logs/godot.log"
	var result: Array[String] = []
	if not FileAccess.file_exists(log_path):
		return result
	var file := FileAccess.open(log_path, FileAccess.READ)
	if file == null:
		return result
	var content := file.get_as_text()
	var lines := content.split("\n")
	var start := maxi(0, lines.size() - 50)
	for i in range(start, lines.size()):
		var line := lines[i].strip_edges()
		if line != "":
			result.append(lines[i])
	return result
