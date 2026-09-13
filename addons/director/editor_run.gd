@tool

const OpsUtil = preload("res://addons/director/ops/ops_util.gd")
const SAVE_BEFORE_RUNNING := "run/auto_save/save_before_running"
const ACTIONS := ["start", "stop", "restart", "status"]
const PRESENTATIONS := ["automated", "deferred", "interactive"]
const LAUNCH_ENVIRONMENT := "THEATRE_LAUNCH_JSON"
static var _last_launch: Dictionary = {}
static var _last_target := ""


static func dispatch(params: Dictionary) -> Dictionary:
	var action: String = params.get("action", "")
	var scene_path: String = params.get("scene_path", "")
	var presentation: String = params.get("presentation", "")
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
		if params.has("presentation"):
			return OpsUtil._error("presentation is only valid for start and restart", "editor_run", {})
		if params.has("launch"):
			return OpsUtil._error("launch is only valid for start and restart", "editor_run", {})
		if scene_path != "":
			return OpsUtil._error(
				"scene_path is only valid for start and restart",
				"editor_run", {"action": action, "scene_path": scene_path})
	if presentation != "" and presentation not in PRESENTATIONS:
		return OpsUtil._error(
			"presentation must be one of: automated, deferred, interactive",
			"editor_run", {"presentation": presentation})

	var previously_playing := EditorInterface.get_playing_scene().trim_prefix("res://")
	if action == "status":
		return _response(action, previously_playing, false)
	if action == "stop":
		if EditorInterface.is_playing_scene():
			EditorInterface.stop_playing_scene()
		_last_launch = {}
		_last_target = ""
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
	var options: Dictionary = params.get("launch", {})
	var resolved: Dictionary = {}
	var launch_path := full_path
	if FileAccess.file_exists("res://addons/stage/launch_policy.gd"):
		var policy = load("res://addons/stage/launch_policy.gd")
		var result: Dictionary = policy.resolve_launch(options)
		if not result.get("ok", false):
			return OpsUtil._error(str(result.error), "editor_run", {})
		resolved = result.config
		if resolved.startup.enabled or resolved.play.enabled or resolved.observe:
			if not ProjectSettings.has_setting("autoload/StageRuntime"):
				return OpsUtil._error("Stage is not wired into this project. Ask the user before installing or enabling it.", "editor_run", {})
		if resolved.startup.enabled:
			launch_path = "res://addons/stage/capture_bootstrap.tscn"
			if not FileAccess.file_exists(launch_path):
				return OpsUtil._error("Startup bootstrap missing. Deploy the matching Stage addon.", "editor_run", {})
	elif not options.is_empty():
		return OpsUtil._error("Stage options were supplied but Stage is missing. Omit launch options for an ordinary run, or ask the user before installing Stage for observation/capture.", "editor_run", {})

	if action == "restart" and EditorInterface.is_playing_scene():
		EditorInterface.stop_playing_scene()

	# Native play normally saves open work according to this editor preference.
	# Director's contract is explicit-save only, so suppress that behavior solely
	# for the synchronous launch request and restore the in-memory value at once.
	var settings := EditorInterface.get_editor_settings()
	var save_before_running = settings.get_setting(SAVE_BEFORE_RUNNING)
	settings.set_setting(SAVE_BEFORE_RUNNING, false)
	var had_environment := OS.has_environment(LAUNCH_ENVIRONMENT)
	var previous_environment := OS.get_environment(LAUNCH_ENVIRONMENT)
	OS.set_environment(LAUNCH_ENVIRONMENT, JSON.stringify({"options": options, "resolved": resolved if not resolved.is_empty() else null, "target_scene": full_path}))
	EditorInterface.play_custom_scene(launch_path)
	if had_environment:
		OS.set_environment(LAUNCH_ENVIRONMENT, previous_environment)
	else:
		OS.unset_environment(LAUNCH_ENVIRONMENT)
	settings.set_setting(SAVE_BEFORE_RUNNING, save_before_running)
	_last_launch = resolved
	_last_target = scene_path.trim_prefix("res://")
	return _response(action, scene_path.trim_prefix("res://"), true)


static func _response(action: String, scene_path: String, launch_requested: bool) -> Dictionary:
	return {"success": true, "data": {
		"action": action,
		"scene_path": scene_path,
		"launch_requested": launch_requested,
		"game_running": EditorInterface.is_playing_scene(),
		"playing_scene": EditorInterface.get_playing_scene().trim_prefix("res://"),
		"launch": _last_launch if not _last_launch.is_empty() else null,
		"target_scene": _last_target if EditorInterface.is_playing_scene() else "",
	}}
