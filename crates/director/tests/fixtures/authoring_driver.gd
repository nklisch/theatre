@tool
extends EditorPlugin

## Test-only editor driver: human history/input and live-state inspection.
## Director edits still travel through the actual separately enabled plugin.
const Codec = preload("res://addons/director/message_codec.gd")
const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")
var server := TCPServer.new()
var client: StreamPeerTCP
var buffer := PackedByteArray()
var busy := false
var initialized := false
var held_animation: Animation

func _enter_tree() -> void:
	server.listen(int(OS.get_environment("DIRECTOR_DRIVER_PORT")), "127.0.0.1")

func _exit_tree() -> void:
	server.stop()

func _process(_delta: float) -> void:
	if server.is_connection_available():
		client = server.take_connection()
		buffer.clear()
	if client == null or busy:
		return
	client.poll()
	if client.get_available_bytes() > 0:
		buffer.append_array(client.get_data(client.get_available_bytes())[1])
	var decoded := Codec.try_decode(buffer)
	if decoded[1] == 0:
		return
	buffer = buffer.slice(decoded[1])
	busy = true
	var result: Dictionary = await command(decoded[0].operation, decoded[0].get("params", {}))
	client.put_data(Codec.encode(result))
	busy = false

func command(operation: String, params: Dictionary) -> Dictionary:
	match operation:
		"ping":
			return {"success": true, "data": {"backend": "editor", "project_path": ProjectSettings.globalize_path("res://"), "process_id": OS.get_process_id()}}
		"input_readiness":
			return {"success": true, "data": input_readiness()}
		"native_edit":
			native_edit(params.operation)
		"prepare":
			if not initialized:
				create_scenes()
				EditorInterface.open_scene_from_path("res://a.tscn")
				await get_tree().process_frame
				human_edit("res://a.tscn", 10.0)
				EditorInterface.open_scene_from_path("res://b.tscn")
				await get_tree().process_frame
				human_edit("res://b.tscn", 20.0)
				initialized = true
		"activate":
			EditorInterface.open_scene_from_path("res://" + params.scene_path)
			await get_tree().process_frame
		"shortcut":
			EditorInterface.open_scene_from_path("res://" + params.scene_path)
			await get_tree().process_frame
			var readiness := input_readiness()
			if readiness.scanning or readiness.input_blocked:
				return {"success": false, "error": "Editor became busy before shortcut: " + str(readiness)}
			# A diagnostic-only override reproduces the original released-focus
			# path on another engine version without altering native key bindings.
			if params.get("focus_scene", OS.get_environment("THEATRE_AUTHORING_RELEASE_CONTROL_FOCUS") != "1"):
				# Exercise the scene's native shortcut context, not an editor with no
				# focused control. This stays within the fixture's own editor window.
				var scene_control: Control = EditorInterface.get_editor_main_screen().find_children("*", "CanvasItemEditorViewport", true, false)[0]
				scene_control.grab_focus()
				if EditorInterface.get_base_control().get_viewport().gui_get_focus_owner() != scene_control:
					return {"success": false, "error": "Could not focus native 2D scene viewport"}
			else:
				EditorInterface.get_base_control().get_viewport().gui_release_focus()
			var event := InputEventKey.new()
			event.keycode = KEY_Z
			event.ctrl_pressed = OS.get_name() != "macOS"
			event.meta_pressed = OS.get_name() == "macOS"
			event.shift_pressed = params.get("redo", false)
			event.pressed = true
			Input.parse_input_event(event)
			await get_tree().process_frame
			# Buffered input retains the event object; don't mutate the press.
			var release := event.duplicate() as InputEventKey
			release.pressed = false
			Input.parse_input_event(release)
			await get_tree().process_frame
		"ephemeral_group":
			var root := EditorInterface.get_edited_scene_root()
			root.get_node("Human").add_to_group("ephemeral", false)
		"unowned_child":
			var child := Node2D.new()
			child.name = "Unowned"
			EditorInterface.get_edited_scene_root().get_node("Human").add_child(child)
		"hold_animation":
			held_animation = load("res://animation.tres")
		"dirty_resource":
			var material = load("res://external.tres")
			material.albedo_color = Color.RED
			material.emit_changed()
		"reopen":
			# Native reload is deliberately test-only, after Director's explicit save.
			EditorInterface.reload_scene_from_path("res://" + params.scene_path)
			await get_tree().process_frame
		"break_save":
			DirAccess.remove_absolute("res://a.tscn")
			DirAccess.make_dir_absolute("res://a.tscn")
		"inspect":
			pass
	return inspect(params.get("scene_path", "a.tscn"), params.get("node_path", "."))

func input_readiness() -> Dictionary:
	# Godot's EditorNode enables _input only while its progress UI is consuming
	# keys. That interception can outlast is_scanning(), so observe both before
	# injecting native shortcuts. The base control's parent is the EditorNode.
	return {
		"scanning": EditorInterface.get_resource_filesystem().is_scanning(),
		"input_blocked": EditorInterface.get_base_control().get_parent().is_processing_input(),
		"accessibility_enabled": get_tree().is_accessibility_enabled(),
		"window_focus": get_window().has_focus(),
		"frames_per_second": Engine.get_frames_per_second(),
		"low_processor_sleep_usec": OS.low_processor_usage_mode_sleep_usec,
	}

# Diagnostic control: no Director plugin or mutation implementation participates.
# Mirror the native edits leading up to the observed AccessKit failure, while
# keeping native keyboard undo and the same objects/tabs as the Director journey.
func native_edit(operation: String) -> void:
	assert(not EditorInterface.is_plugin_enabled("director"))
	var previous := EditorInterface.get_edited_scene_root().scene_file_path
	EditorInterface.open_scene_from_path("res://a.tscn")
	var root := EditorInterface.get_edited_scene_root()
	var manager := get_undo_redo()
	manager.create_action("Native comparison: " + operation, UndoRedo.MERGE_DISABLE, root)
	match operation:
		"node_add", "scene_add_instance":
			var added: Node = Node2D.new() if operation == "node_add" else load("res://instance.tscn").instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE)
			added.name = "Added"
			var parent := root.get_node("Parent")
			manager.add_do_method(parent, "add_child", added)
			manager.add_do_property(added, "owner", root)
			manager.add_undo_method(parent, "remove_child", added)
			manager.add_do_reference(added)
		"node_remove":
			var node := root.get_node("Human")
			manager.add_do_method(root, "remove_child", node)
			manager.add_undo_method(root, "add_child", node)
			manager.add_undo_method(root, "move_child", node, node.get_index())
			manager.add_undo_property(node, "owner", root)
			manager.add_undo_reference(node)
		"node_reparent":
			var node := root.get_node("Human")
			manager.add_do_method(node, "reparent", root.get_node("Parent"), true)
			manager.add_do_property(node, "name", "Moved")
			manager.add_undo_method(node, "reparent", root, true)
			manager.add_undo_property(node, "name", "Human")
			manager.add_undo_method(root, "move_child", node, node.get_index())
		"node_set_properties":
			var node: Node2D = root.get_node("Human")
			manager.add_do_property(node, "position", Vector2(42, 2))
			manager.add_undo_property(node, "position", node.position)
	manager.commit_action()
	EditorInterface.open_scene_from_path(previous)

func human_edit(path: String, x: float) -> void:
	var root := EditorInterface.get_edited_scene_root()
	assert(root.scene_file_path == path)
	var node: Node2D = root.get_node("Human")
	var manager := get_undo_redo()
	manager.create_action("Human move", UndoRedo.MERGE_DISABLE, root)
	manager.add_do_property(node, "position", Vector2(x, 0))
	manager.add_undo_property(node, "position", Vector2.ZERO)
	manager.commit_action()

func create_scenes() -> void:
	var external := StandardMaterial3D.new()
	ResourceSaver.save(external, "res://external.tres")
	external = load("res://external.tres")
	var nested := Node2D.new()
	nested.name = "Instance"
	var leaf := Node2D.new()
	leaf.name = "Leaf"
	nested.add_child(leaf)
	leaf.owner = nested
	assert(SceneEdit.save_root(nested, "instance.tscn").success)
	nested.free()
	for scene in ["a", "b"]:
		var root := Node2D.new()
		root.name = scene.to_upper()
		for name in ["Human", "Parent", "Sibling"]:
			var child := Node2D.new()
			child.name = name
			root.add_child(child)
			child.owner = root
		var body := StaticBody2D.new()
		body.name = "Body"
		root.add_child(body)
		body.owner = root
		var shape := CollisionShape2D.new()
		shape.name = "Shape"
		body.add_child(shape)
		shape.owner = root
		var button := Button.new()
		button.name = "Button"
		root.add_child(button)
		button.owner = root
		var mesh := MeshInstance3D.new()
		mesh.name = "Mesh"
		mesh.material_override = external
		root.add_child(mesh)
		mesh.owner = root
		var tiles := TileMapLayer.new()
		tiles.name = "Tiles"
		tiles.tile_set = TileSet.new()
		var atlas := TileSetAtlasSource.new()
		var image := Image.create(16, 16, false, Image.FORMAT_RGBA8)
		image.fill(Color.WHITE)
		atlas.texture = ImageTexture.create_from_image(image)
		atlas.create_tile(Vector2i.ZERO)
		tiles.tile_set.add_source(atlas, 0)
		tiles.set_cell(Vector2i.ZERO, 0, Vector2i.ZERO, 0)
		root.add_child(tiles)
		tiles.owner = root
		var grid := GridMap.new()
		grid.name = "Grid"
		grid.mesh_library = MeshLibrary.new()
		grid.mesh_library.create_item(0)
		grid.mesh_library.set_item_mesh(0, BoxMesh.new())
		grid.set_cell_item(Vector3i.ZERO, 0, 3)
		root.add_child(grid)
		grid.owner = root
		assert(SceneEdit.save_root(root, scene + ".tscn").success)
		root.free()

func inspect(scene_path: String, node_path: String) -> Dictionary:
	var root: Node
	for candidate in EditorInterface.get_open_scene_roots():
		if candidate.scene_file_path == "res://" + scene_path:
			root = candidate
	if root == null:
		return {"success": false, "error": "Scene not open"}
	var node := root.get_node_or_null(NodePath(node_path))
	var data := {
		"active_scene": EditorInterface.get_edited_scene_root().scene_file_path,
		"dirty": EditorInterface.get_unsaved_scenes(),
		"exists": node != null,
		"save_before_running": EditorInterface.get_editor_settings().get_setting("run/auto_save/save_before_running"),
	}
	if held_animation != null:
		data["cached_tracks"] = held_animation.get_track_count()
	if node != null:
		data["instance_id"] = node.get_instance_id()
		data["owner_id"] = node.owner.get_instance_id() if node.owner != null else 0
		data["index"] = node.get_index()
		data["groups"] = node.get_groups()
		var packed := PackedScene.new()
		if packed.pack(node) == OK:
			data["persistent_groups"] = packed.get_state().get_node_groups(0)
		data["meta"] = {}
		for key in node.get_meta_list():
			data.meta[key] = node.get_meta(key)
		if node is Node2D:
			data["position"] = [node.position.x, node.position.y]
		if node is MeshInstance3D and node.material_override != null:
			data["material_path"] = node.material_override.resource_path
			data["material_red"] = node.material_override.albedo_color == Color.RED
		if node is CollisionObject2D:
			data["collision_layer"] = node.collision_layer
		if node is CollisionShape2D:
			data["shape"] = node.shape.get_class() if node.shape else ""
		if node.get_script() != null:
			data["script"] = node.get_script().resource_path
			data["speed"] = node.get("speed")
			data["limited"] = node.get("limited")
	return {"success": true, "data": data}
