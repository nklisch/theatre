@tool

const OpsUtil = preload("res://addons/director/ops/ops_util.gd")
const SAVE_BEFORE_RUNNING := "run/auto_save/save_before_running"
const ACTIONS := ["start", "stop", "restart", "status"]


static func dispatch(params: Dictionary) -> Dictionary:
	var action: String = params.get("action", "")
	var scene_path: String = params.get("scene_path", "")
	if action not in ACTIONS:
		return OpsUtil._error(
			"action must be one of: start, stop, restart, status",
			"editor_run", {"action": action})
	if action in ["start", "restart"]:
		if scene_path == "":
			return OpsUtil._error(
				"scene_path is required for %s" % action,
				"editor_run", {"action": action})
	else:
		if scene_path != "":
			return OpsUtil._error(
				"scene_path is only valid for start and restart",
				"editor_run", {"action": action, "scene_path": scene_path})

	var previously_playing := EditorInterface.get_playing_scene().trim_prefix("res://")
	if action == "status":
		return _response(action, previously_playing, false)
	if action == "stop":
		if EditorInterface.is_playing_scene():
			EditorInterface.stop_playing_scene()
		return _response(action, previously_playing, false)
	if action == "start" and EditorInterface.is_playing_scene():
		return OpsUtil._error(
			"A scene is already running; use restart or stop first",
			"editor_run", {"action": action, "playing_scene": previously_playing})

	var full_path := ("res://" + scene_path.trim_prefix("res://")).simplify_path()
	if not FileAccess.file_exists(full_path):
		return OpsUtil._error(
			"Saved scene not found: " + scene_path,
			"editor_run", {"action": action, "scene_path": scene_path})
	var saved_scene := ResourceLoader.load(full_path, "PackedScene", ResourceLoader.CACHE_MODE_IGNORE)
	if not saved_scene is PackedScene:
		return OpsUtil._error(
			"Path is not a loadable saved scene: " + scene_path,
			"editor_run", {"action": action, "scene_path": scene_path})

	if action == "restart" and EditorInterface.is_playing_scene():
		EditorInterface.stop_playing_scene()

	# Native play normally saves open work according to this editor preference.
	# Director's contract is explicit-save only, so suppress that behavior solely
	# for the synchronous launch request and restore the in-memory value at once.
	var settings := EditorInterface.get_editor_settings()
	var save_before_running = settings.get_setting(SAVE_BEFORE_RUNNING)
	settings.set_setting(SAVE_BEFORE_RUNNING, false)
	EditorInterface.play_custom_scene(full_path)
	settings.set_setting(SAVE_BEFORE_RUNNING, save_before_running)
	return _response(action, scene_path.trim_prefix("res://"), true)


static func _response(action: String, scene_path: String, launch_requested: bool) -> Dictionary:
	return {"success": true, "data": {
		"action": action,
		"scene_path": scene_path,
		"launch_requested": launch_requested,
		"game_running": EditorInterface.is_playing_scene(),
		"playing_scene": EditorInterface.get_playing_scene().trim_prefix("res://"),
	}}
