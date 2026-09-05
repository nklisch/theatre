@tool
extends EditorPlugin
const EditorFeedback := preload("res://addons/director/editor_feedback.gd")

func _enter_tree() -> void:
	call_deferred("run")

func run() -> void:
	var is_3d := OS.get_environment("FEEDBACK_EDITOR_MODE") == "3D"
	var node: Node = Node3D.new() if is_3d else Node2D.new()
	node.name = "SelectedForFeedback"
	var scene := PackedScene.new()
	assert(scene.pack(node) == OK)
	assert(ResourceSaver.save(scene, "res://feedback_scene.tscn") == OK)
	node.free()
	EditorInterface.open_scene_from_path("res://feedback_scene.tscn")
	EditorInterface.set_main_screen_editor("3D" if is_3d else "2D")
	for frame in range(20):
		await get_tree().process_frame
	var root := EditorInterface.get_edited_scene_root()
	assert(root != null)
	var position: Variant = Vector3(22, 44, 0) if is_3d else Vector2(22, 44)
	root.set("position", position)
	EditorInterface.mark_scene_as_unsaved()
	EditorInterface.get_selection().add_node(root)
	var dirty_before := EditorInterface.get_unsaved_scenes()
	var selected_before := EditorInterface.get_selection().get_selected_nodes()
	await RenderingServer.frame_post_draw
	var feedback := EditorFeedback.new()
	feedback.main_screen = "3D" if is_3d else "2D"
	feedback.share(self)
	await get_tree().process_frame
	assert(feedback.composer.size.y <= DisplayServer.screen_get_usable_rect().size.y, "Queue button must fit on screen")
	assert(feedback.composer.get_ok_button().is_visible_in_tree())
	assert(str(feedback.composer._composition.item.surface).begins_with("editor_3d" if is_3d else "editor_2d"))
	assert(feedback.composer._composition.item.capture.status == "available")
	feedback.composer._note.text = "Editor selection and viewport"
	feedback.composer._queue()
	assert(EditorInterface.get_unsaved_scenes() == dirty_before)
	assert(EditorInterface.get_selection().get_selected_nodes() == selected_before)
	assert(root.get("position") == position)
	print("FEEDBACK_EDITOR_OK")
	get_tree().quit()
