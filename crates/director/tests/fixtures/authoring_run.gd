extends SceneTree

func _init() -> void:
	var scene: PackedScene = load("res://a.tscn")
	var instance := scene.instantiate()
	root.add_child(instance)
	current_scene = instance
	var human: Node2D = instance.get_node("Human")
	var mesh: MeshInstance3D = instance.get_node("Mesh")
	var valid: bool = human.position == Vector2(30, 4) and human.get_meta("later", false)
	valid = valid and mesh.material_override.albedo_color == Color.WHITE
	print(JSON.stringify({"saved_scene_ran": valid}))
	quit(0 if valid else 1)
