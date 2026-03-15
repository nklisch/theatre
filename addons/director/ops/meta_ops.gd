class_name MetaOps

const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")
const TileMapOps = preload("res://addons/director/ops/tilemap_ops.gd")
const GridMapOps = preload("res://addons/director/ops/gridmap_ops.gd")
const AnimationOps = preload("res://addons/director/ops/animation_ops.gd")
const PhysicsOps = preload("res://addons/director/ops/physics_ops.gd")
const ShaderOps = preload("res://addons/director/ops/shader_ops.gd")
const ProjectOps = preload("res://addons/director/ops/project_ops.gd")
const SignalOps = preload("res://addons/director/ops/signal_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_batch(params: Dictionary) -> Dictionary:
	## Execute multiple operations in sequence within a single Godot process.
	##
	## Params:
	##   operations: Array[{ operation: String, params: Dictionary }]
	##   stop_on_error: bool (default true)
	## Returns:
	##   { success, data: { results: [...], completed: int, failed: int } }
	var operations: Array = params.get("operations", [])
	var stop_on_error: bool = params.get("stop_on_error", true)

	if operations.is_empty():
		return OpsUtil._error("operations array is required and must not be empty",
			"batch", params)

	var results: Array = []
	var completed: int = 0
	var failed: int = 0

	for entry in operations:
		var operation: String = entry.get("operation", "")
		var op_params: Dictionary = entry.get("params", {})

		if operation == "":
			var err_result = {
				"operation": "", "success": false,
				"error": "operation name is required"
			}
			results.append(err_result)
			failed += 1
			if stop_on_error:
				break
			continue

		if operation == "batch":
			var err_result = {
				"operation": "batch", "success": false,
				"error": "batch cannot be nested"
			}
			results.append(err_result)
			failed += 1
			if stop_on_error:
				break
			continue

		var result: Dictionary = _dispatch_single(operation, op_params)
		var success: bool = result.get("success", false)

		results.append({
			"operation": operation,
			"success": success,
			"data": result.get("data", null) if success else null,
			"error": result.get("error", null) if not success else null,
		})

		if success:
			completed += 1
		else:
			failed += 1
			if stop_on_error:
				break

	return {"success": failed == 0, "data": {
		"results": results,
		"completed": completed,
		"failed": failed,
	}}


static func op_scene_diff(params: Dictionary) -> Dictionary:
	## Compare two scene files structurally.
	##
	## Params:
	##   scene_a: String  — path relative to project, or git ref (e.g. "HEAD:scenes/player.tscn")
	##   scene_b: String  — path relative to project, or git ref
	## Returns:
	##   { success, data: { added: [...], removed: [...], moved: [...],
	##     changed: [...] } }
	var scene_a: String = params.get("scene_a", "")
	var scene_b: String = params.get("scene_b", "")

	if scene_a == "":
		return OpsUtil._error("scene_a is required", "scene_diff", params)
	if scene_b == "":
		return OpsUtil._error("scene_b is required", "scene_diff", params)

	# Resolve both sources (handles git ref syntax)
	var src_a = _resolve_scene_source(scene_a)
	if src_a.error != "":
		return OpsUtil._error(src_a.error, "scene_diff", {"scene_a": scene_a})

	var src_b = _resolve_scene_source(scene_b)
	if src_b.error != "":
		_cleanup_temp(src_a)
		return OpsUtil._error(src_b.error, "scene_diff", {"scene_b": scene_b})

	var full_a: String = src_a.path
	var full_b: String = src_b.path

	if not ResourceLoader.exists(full_a):
		_cleanup_temp(src_a)
		_cleanup_temp(src_b)
		return OpsUtil._error("Scene not found: " + scene_a, "scene_diff", {"scene_a": scene_a})
	if not ResourceLoader.exists(full_b):
		_cleanup_temp(src_a)
		_cleanup_temp(src_b)
		return OpsUtil._error("Scene not found: " + scene_b, "scene_diff", {"scene_b": scene_b})

	var packed_a: PackedScene = load(full_a)
	var packed_b: PackedScene = load(full_b)
	var root_a = packed_a.instantiate()
	var root_b = packed_b.instantiate()

	var map_a: Dictionary = {}
	var map_b: Dictionary = {}
	_collect_node_map(root_a, root_a, map_a)
	_collect_node_map(root_b, root_b, map_b)

	var added: Array = []
	var removed: Array = []
	var changed: Array = []

	for path in map_b:
		if path not in map_a:
			added.append({"node_path": path, "type": map_b[path].type})

	for path in map_a:
		if path not in map_b:
			removed.append({"node_path": path, "type": map_a[path].type})

	for path in map_a:
		if path in map_b:
			var props_a: Dictionary = map_a[path].properties
			var props_b: Dictionary = map_b[path].properties
			if map_a[path].type != map_b[path].type:
				changed.append({
					"node_path": path,
					"property": "_type",
					"old_value": map_a[path].type,
					"new_value": map_b[path].type,
				})
			var all_keys: Dictionary = {}
			for k in props_a:
				all_keys[k] = true
			for k in props_b:
				all_keys[k] = true
			for key in all_keys:
				var val_a = props_a.get(key, null)
				var val_b = props_b.get(key, null)
				if not _values_equal(val_a, val_b):
					changed.append({
						"node_path": path,
						"property": key,
						"old_value": SceneOps._serialize_value(val_a) \
							if val_a != null else null,
						"new_value": SceneOps._serialize_value(val_b) \
							if val_b != null else null,
					})

	root_a.free()
	root_b.free()

	_cleanup_temp(src_a)
	_cleanup_temp(src_b)

	return {"success": true, "data": {
		"added": added,
		"removed": removed,
		"moved": [],
		"changed": changed,
	}}


static func _resolve_scene_source(scene_ref: String) -> Dictionary:
	## Resolve a scene reference to a res:// path.
	## Supports git ref syntax: "HEAD:scenes/player.tscn" or "abc123:scenes/player.tscn"
	## Returns { path: String, is_temp: bool, temp_path: String, error: String }
	if ":" in scene_ref and not scene_ref.begins_with("res://"):
		# Git ref syntax: split on first ":"
		var colon_idx: int = scene_ref.find(":")
		var git_ref: String = scene_ref.substr(0, colon_idx)
		var file_path: String = scene_ref.substr(colon_idx + 1)

		var project_root: String = ProjectSettings.globalize_path("res://")

		var output: Array = []
		var exit_code: int = OS.execute("git", [
			"-C", project_root,
			"show", git_ref + ":" + file_path,
		], output, true)

		if exit_code != 0:
			return {"path": "", "is_temp": false, "temp_path": "", "error": "Git ref not found: " + scene_ref}

		# Write content to a temp file
		if not DirAccess.dir_exists_absolute(ProjectSettings.globalize_path("res://tmp")):
			DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("res://tmp"))

		var temp_name: String = "res://tmp/_scene_diff_" + str(Time.get_ticks_msec()) + ".tscn"
		var f = FileAccess.open(temp_name, FileAccess.WRITE)
		if f == null:
			return {"path": "", "is_temp": false, "temp_path": "", "error": "Failed to create temp file for git ref: " + scene_ref}
		f.store_string(output[0])
		f.close()

		return {"path": temp_name, "is_temp": true, "temp_path": temp_name, "error": ""}
	else:
		return {"path": "res://" + scene_ref, "is_temp": false, "temp_path": "", "error": ""}


static func _cleanup_temp(src: Dictionary) -> void:
	## Remove a temp file created by _resolve_scene_source.
	if src.get("is_temp", false) and src.get("temp_path", "") != "":
		DirAccess.remove_absolute(ProjectSettings.globalize_path(src.temp_path))


static func _dispatch_single(operation: String, params: Dictionary) -> Dictionary:
	match operation:
		"scene_create": return SceneOps.op_scene_create(params)
		"scene_read": return SceneOps.op_scene_read(params)
		"node_add": return NodeOps.op_node_add(params)
		"node_set_properties": return NodeOps.op_node_set_properties(params)
		"node_remove": return NodeOps.op_node_remove(params)
		"node_reparent": return NodeOps.op_node_reparent(params)
		"scene_list": return SceneOps.op_scene_list(params)
		"scene_add_instance": return SceneOps.op_scene_add_instance(params)
		"resource_read": return ResourceOps.op_resource_read(params)
		"material_create": return ResourceOps.op_material_create(params)
		"shape_create": return ResourceOps.op_shape_create(params)
		"style_box_create": return ResourceOps.op_style_box_create(params)
		"resource_duplicate": return ResourceOps.op_resource_duplicate(params)
		"tilemap_set_cells": return TileMapOps.op_tilemap_set_cells(params)
		"tilemap_get_cells": return TileMapOps.op_tilemap_get_cells(params)
		"tilemap_clear": return TileMapOps.op_tilemap_clear(params)
		"gridmap_set_cells": return GridMapOps.op_gridmap_set_cells(params)
		"gridmap_get_cells": return GridMapOps.op_gridmap_get_cells(params)
		"gridmap_clear": return GridMapOps.op_gridmap_clear(params)
		"animation_create": return AnimationOps.op_animation_create(params)
		"animation_add_track": return AnimationOps.op_animation_add_track(params)
		"animation_read": return AnimationOps.op_animation_read(params)
		"animation_remove_track": return AnimationOps.op_animation_remove_track(params)
		"physics_set_layers": return PhysicsOps.op_physics_set_layers(params)
		"physics_set_layer_names": return PhysicsOps.op_physics_set_layer_names(params)
		"visual_shader_create": return ShaderOps.op_visual_shader_create(params)
		"scene_diff": return op_scene_diff(params)
		"autoload_add": return ProjectOps.op_autoload_add(params)
		"autoload_remove": return ProjectOps.op_autoload_remove(params)
		"project_settings_set": return ProjectOps.op_project_settings_set(params)
		"project_reload": return ProjectOps.op_project_reload(params)
		"editor_status": return ProjectOps.op_editor_status(params)
		"uid_get": return ProjectOps.op_uid_get(params)
		"uid_update_project": return ProjectOps.op_uid_update_project(params)
		"export_mesh_library": return ProjectOps.op_export_mesh_library(params)
		"signal_connect": return SignalOps.op_signal_connect(params)
		"signal_disconnect": return SignalOps.op_signal_disconnect(params)
		"signal_list": return SignalOps.op_signal_list(params)
		"node_set_groups": return NodeOps.op_node_set_groups(params)
		"node_set_script": return NodeOps.op_node_set_script(params)
		"node_set_meta": return NodeOps.op_node_set_meta(params)
		"node_find": return NodeOps.op_node_find(params)
		_:
			return OpsUtil._error("Unknown operation: " + operation, operation, {})


static func _collect_node_map(node: Node, root: Node, result: Dictionary) -> void:
	## Collect all nodes into a path→{type, properties} dictionary.
	var path: String = str(root.get_path_to(node))
	result[path] = {
		"type": node.get_class(),
		"properties": SceneOps._get_serializable_properties(node),
	}
	for child in node.get_children():
		_collect_node_map(child, root, result)


static func _values_equal(a, b) -> bool:
	## Deep comparison that handles Godot types correctly.
	if typeof(a) != typeof(b):
		return false
	if a is Dictionary and b is Dictionary:
		if a.size() != b.size():
			return false
		for key in a:
			if key not in b or not _values_equal(a[key], b[key]):
				return false
		return true
	if a is Array and b is Array:
		if a.size() != b.size():
			return false
		for i in range(a.size()):
			if not _values_equal(a[i], b[i]):
				return false
		return true
	return a == b
