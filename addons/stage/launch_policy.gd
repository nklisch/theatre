extends RefCounted
## Reads private preferences without changing project settings or capture state.

const ENVIRONMENT := "THEATRE_LAUNCH_JSON"


static func resolve_launch(explicit: Dictionary = {}) -> Dictionary:
	if not ClassDB.class_exists(&"StageCapturePolicy"):
		return {"ok": false, "error": "Stage capture policy is unavailable. Deploy the matching Stage extension."}
	var contents: Array[String] = []
	for path in ["res://stage.toml", OS.get_config_dir().path_join("theatre/settings.toml"), "res://stage.local.toml"]:
		if not FileAccess.file_exists(path):
			contents.append("")
			continue
		var file := FileAccess.open(path, FileAccess.READ)
		if file == null:
			return {"ok": false, "error": "Cannot read Theatre configuration: " + error_string(FileAccess.get_open_error())}
		contents.append(file.get_as_text())
		file.close()
	var result = JSON.parse_string(ClassDB.class_call_static(&"StageCapturePolicy", &"resolve_launch",
		contents[0], contents[1], contents[2], JSON.stringify(explicit)))
	if not result is Dictionary:
		return {"ok": false, "error": "Stage capture policy returned invalid data."}
	return result


static func runtime_request() -> Dictionary:
	var request: Dictionary = {"options": {}, "target_scene": ""}
	var environment := OS.get_environment(ENVIRONMENT)
	if not environment.is_empty():
		var parsed = JSON.parse_string(environment)
		if not parsed is Dictionary or not parsed.get("options", {}) is Dictionary or not parsed.get("target_scene", "") is String:
			return {"ok": false, "error": "Invalid THEATRE_LAUNCH_JSON request."}
		request = parsed
	var resolved: Variant = request.get("resolved")
	if resolved != null and not resolved is Dictionary:
		return {"ok": false, "error": "Invalid resolved Theatre launch plan."}
	var options: Dictionary = {} if resolved != null else request.get("options", {}).duplicate(true)
	for argument in OS.get_cmdline_user_args():
		if not argument.begins_with("--theatre-"):
			continue
		var split := argument.trim_prefix("--theatre-").split("=", true, 1)
		if split.size() != 2:
			return {"ok": false, "error": "Theatre arguments require --theatre-name=value."}
		var key: String = split[0]
		var value: String = split[1]
		match key:
			"options":
				var parsed: Variant = JSON.parse_string(value)
				if not parsed is Dictionary:
					return {"ok": false, "error": "--theatre-options expects a LaunchOptions JSON object."}
				options.merge(parsed, true)
			"startup", "play":
				if not options.has(key):
					options[key] = {}
				options[key]["preset"] = value
			"observe":
				if value not in ["on", "off"]:
					return {"ok": false, "error": "--theatre-observe expects on or off."}
				options[key] = value == "on"
			"operator", "readiness", "play-start":
				options[key.replace("-", "_")] = value
			"target":
				request["target_scene"] = value
			_:
				return {"ok": false, "error": "Unknown Theatre argument: " + key}
	var result: Dictionary
	if resolved != null:
		result = JSON.parse_string(ClassDB.class_call_static(&"StageCapturePolicy", &"resolve_runtime", JSON.stringify(resolved), JSON.stringify(options)))
	else:
		result = resolve_launch(options)
	if result.get("ok", false):
		result["target_scene"] = request.get("target_scene", "")
	return result
