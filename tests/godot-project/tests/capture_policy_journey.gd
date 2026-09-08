extends SceneTree

var failures: Array[String] = []


func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	call_deferred("_exercise")


func check(condition: bool, message: String) -> void:
	if not condition:
		failures.append(message)


func _exercise() -> void:
	var policy = load("res://addons/stage/launch_policy.gd")
	var result: Dictionary = policy.resolve_launch({})
	check(result.get("ok", false), "native policy resolves: " + str(result))
	if result.get("ok", false):
		check(not result.config.observe and not result.config.startup.enabled and not result.config.play.enabled, "ordinary policy is off")
	result = policy.resolve_launch({"operator": "agent", "startup": {"preset": "minimal"}, "play": {"preset": "heavy", "images": false}})
	check(result.get("ok", false), "phase policy resolves: " + str(result))
	if result.get("ok", false):
		check(result.config.play_start == "ready", "agent starts at readiness")
		check(result.config.startup.metrics and not result.config.startup.spatial and not result.config.startup.images, "metrics-only startup")
		check(result.config.play.spatial and not result.config.play.images, "explicit play override")
		var frozen: Dictionary = result.config.duplicate(true)
		OS.set_environment("THEATRE_LAUNCH_JSON", JSON.stringify({"resolved": frozen}))
		var changed := FileAccess.open("res://stage.toml", FileAccess.WRITE)
		changed.store_string("[launch]\nthis_is_invalid=true\n")
		changed.close()
		var inherited: Dictionary = policy.runtime_request()
		check(inherited.get("ok", false) and inherited.get("config") == frozen, "runtime uses accepted editor plan without rereading changed preferences")
		DirAccess.remove_absolute(ProjectSettings.globalize_path("res://stage.toml"))
		OS.unset_environment("THEATRE_LAUNCH_JSON")
	result = policy.resolve_launch({"play": {"preset": "standard"}})
	check(not result.get("ok", false), "missing scope is rejected")
	check(root.get_child_count() == 0, "policy resolution creates no runtime nodes")
	for failure in failures:
		push_error(failure)
	print("CAPTURE_POLICY_JOURNEY ", "PASS" if failures.is_empty() else "FAIL")
	quit(0 if failures.is_empty() else 1)
