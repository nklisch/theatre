extends SceneTree
## Live exported-state fixture for snapshot and inspection wire queries.

class ExportBase extends Node2D:
	@export var health: int = 42
	@export var inventory: PackedStringArray = ["key", "coin"]
	var internal_value: int = 99

class ExportSubject extends ExportBase:
	@export var direction: Vector2 = Vector2(2, 3)
	@export var hidden_export: int = 7
	var changed := false

	func _get_property_list() -> Array[Dictionary]:
		return [{"name": "dynamic_after" if changed else "dynamic_before",
			"type": TYPE_INT, "usage": PROPERTY_USAGE_EDITOR | PROPERTY_USAGE_SCRIPT_VARIABLE}]

	func _get(property: StringName) -> Variant:
		if property == &"dynamic_before" and not changed:
			return 123
		if property == &"dynamic_after" and changed:
			return 456
		return null

	func _validate_property(property: Dictionary) -> void:
		# Preserve the current object's usage, not just its script's static list.
		if property.name == "hidden_export" and not changed:
			property.usage &= ~PROPERTY_USAGE_EDITOR
		if property.name == "visible":
			property.usage |= PROPERTY_USAGE_SCRIPT_VARIABLE | PROPERTY_USAGE_EDITOR

	func change_exports() -> void:
		health = 17
		inventory = ["gem"]
		direction = Vector2(-1, 8)
		visible = false
		changed = true
		notify_property_list_changed()

func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	ProjectSettings.set_setting("theatre/stage/dashcam/enabled", false)
	ProjectSettings.set_setting("theatre/stage/display/capture_controls", "hidden")
	var scene := Node2D.new()
	scene.name = "ExportedStateJourney"
	root.add_child(scene)
	current_scene = scene
	var subject := ExportSubject.new()
	subject.name = "Subject"
	scene.add_child(subject)
	var runtime = load("res://addons/stage/runtime.gd").new()
	runtime.name = "StageRuntime"
	root.add_child(runtime)
