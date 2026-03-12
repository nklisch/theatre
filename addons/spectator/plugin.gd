@tool
extends EditorPlugin

var _dock: Control
var _debugger_plugin: EditorDebuggerPlugin


func _enter_tree() -> void:
	_dock = preload("res://addons/spectator/dock.tscn").instantiate()
	add_control_to_dock(DOCK_SLOT_RIGHT_BL, _dock)

	_debugger_plugin = preload("res://addons/spectator/debugger_plugin.gd").new()
	_debugger_plugin._dock = _dock
	add_debugger_plugin(_debugger_plugin)


func _exit_tree() -> void:
	if _debugger_plugin:
		remove_debugger_plugin(_debugger_plugin)
		_debugger_plugin = null
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
	_add_setting("theatre/spectator/connection/port", TYPE_INT, 9077,
		PROPERTY_HINT_RANGE, "1024,65535")
	_add_setting("theatre/spectator/connection/auto_start", TYPE_BOOL, true)
	_add_setting("theatre/spectator/connection/client_idle_timeout_secs", TYPE_INT, 10,
		PROPERTY_HINT_RANGE, "0,3600")
	_add_setting("theatre/spectator/display/show_agent_notifications", TYPE_BOOL, true)
	_add_setting("theatre/spectator/shortcuts/marker_key", TYPE_STRING, "F9",
		PROPERTY_HINT_NONE, "Key name for marker/dashcam clip (e.g. F9). Avoid F5-F11 (Godot editor shortcuts).")
	_add_setting("theatre/spectator/shortcuts/pause_key", TYPE_STRING, "F11",
		PROPERTY_HINT_NONE, "Key name for pausing the game tree (e.g. F11). Avoid F5-F10 (Godot editor shortcuts).")
	_add_setting("theatre/spectator/tracking/default_static_patterns",
		TYPE_PACKED_STRING_ARRAY, PackedStringArray())
	_add_setting("theatre/spectator/tracking/token_hard_cap", TYPE_INT, 5000,
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
