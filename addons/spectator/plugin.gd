@tool
extends EditorPlugin

var _dock: Control


func _enter_tree() -> void:
	_dock = preload("res://addons/spectator/dock.tscn").instantiate()
	add_control_to_dock(DOCK_SLOT_RIGHT_BL, _dock)


func _exit_tree() -> void:
	if _dock:
		remove_control_from_docks(_dock)
		_dock.queue_free()
		_dock = null


func _enable_plugin() -> void:
	_register_settings()
	add_autoload_singleton("SpectatorRuntime", "res://addons/spectator/runtime.gd")


func _disable_plugin() -> void:
	remove_autoload_singleton("SpectatorRuntime")


func _register_settings() -> void:
	_add_setting("spectator/connection/port", TYPE_INT, 9077,
		PROPERTY_HINT_RANGE, "1024,65535")
	_add_setting("spectator/connection/auto_start", TYPE_BOOL, true)
	_add_setting("spectator/recording/storage_path", TYPE_STRING,
		"user://spectator_recordings/")
	_add_setting("spectator/recording/max_frames", TYPE_INT, 36000,
		PROPERTY_HINT_RANGE, "600,360000")
	_add_setting("spectator/recording/capture_interval", TYPE_INT, 1,
		PROPERTY_HINT_RANGE, "1,60")
	_add_setting("spectator/display/show_agent_notifications", TYPE_BOOL, true)
	_add_setting("spectator/display/show_recording_indicator", TYPE_BOOL, true)
	_add_setting("spectator/tracking/default_static_patterns",
		TYPE_PACKED_STRING_ARRAY, PackedStringArray())
	_add_setting("spectator/tracking/token_hard_cap", TYPE_INT, 5000,
		PROPERTY_HINT_RANGE, "500,50000")


func _add_setting(path: String, type: int, default_value: Variant,
		hint: int = PROPERTY_HINT_NONE, hint_string: String = "") -> void:
	if not ProjectSettings.has_setting(path):
		ProjectSettings.set_setting(path, default_value)
	ProjectSettings.set_initial_value(path, default_value)
	ProjectSettings.add_property_info({
		"name": path,
		"type": type,
		"hint": hint,
		"hint_string": hint_string,
	})
