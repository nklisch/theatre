@tool
extends RefCounted

const Feedback := preload("res://addons/theatre_shared/feedback.gd")
const Composer := preload("res://addons/theatre_shared/feedback_composer.gd")
var main_screen := ""
var composer: ConfirmationDialog

func share(parent: Node) -> void:
	if is_instance_valid(composer):
		composer.grab_focus()
		return
	var root := EditorInterface.get_edited_scene_root()
	var selection: Array = []
	for node in EditorInterface.get_selection().get_selected_nodes():
		selection.append({"path": str(node.get_path()), "class": node.get_class()})
	var viewport: Viewport = null
	var surface := "unavailable"
	var screen := main_screen
	# Plugin activation may happen after the initial main-screen signal. Use
	# actual container visibility, not the selected node's class, to recover it.
	if screen.is_empty():
		var view_2d := EditorInterface.get_editor_viewport_2d()
		if view_2d != null and view_2d.get_parent() is Control and view_2d.get_parent().is_visible_in_tree():
			screen = "2D"
		else:
			for index in range(4):
				var view_3d := EditorInterface.get_editor_viewport_3d(index)
				if view_3d != null and view_3d.get_parent() is Control and view_3d.get_parent().is_visible_in_tree():
					screen = "3D"
	if screen == "2D":
		viewport = EditorInterface.get_editor_viewport_2d()
		surface = "editor_2d"
	elif screen == "3D":
		var visible_views: Array[SubViewport] = []
		for index in range(4):
			var candidate := EditorInterface.get_editor_viewport_3d(index)
			if candidate == null or not candidate.get_parent() is Control or not candidate.get_parent().is_visible_in_tree():
				continue
			visible_views.append(candidate)
			if candidate.get_visible_rect().has_point(candidate.get_mouse_position()):
				viewport = candidate
				surface = "editor_3d_%d" % index
		# With split views and no pointer, no documented active-view API tells us
		# which view the human meant. Keep selection/note usable instead of guessing.
		if viewport == null and visible_views.size() == 1:
			viewport = visible_views[0]
			surface = "editor_3d"
	var composition := Feedback.capture(viewport, "editor", root.scene_file_path if root else "", selection, surface)
	composer = Composer.new()
	parent.add_child(composer)
	composer.compose(composition)

func close() -> void:
	if is_instance_valid(composer):
		composer.queue_free()
