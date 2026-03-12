@tool

## Editor-context operation dispatcher.
##
## Routes operations based on whether the target scene is the currently
## active scene in the editor:
##   - Active scene → live tree manipulation via EditorInterface
##   - Non-active/non-open scene → delegate to ops/ + reload/scan
##   - Resource-only operations → delegate to ops/ + scan

const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")
const TileMapOps = preload("res://addons/director/ops/tilemap_ops.gd")
const GridMapOps = preload("res://addons/director/ops/gridmap_ops.gd")
const AnimationOps = preload("res://addons/director/ops/animation_ops.gd")
const PhysicsOps = preload("res://addons/director/ops/physics_ops.gd")
const ShaderOps = preload("res://addons/director/ops/shader_ops.gd")
const MetaOps = preload("res://addons/director/ops/meta_ops.gd")
const ProjectOps = preload("res://addons/director/ops/project_ops.gd")
const SignalOps = preload("res://addons/director/ops/signal_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")

## Scene-targeting operations that can use live tree manipulation.
const SCENE_OPS := [
	"scene_read", "node_add", "node_set_properties", "node_remove",
	"node_reparent", "scene_add_instance",
	"tilemap_set_cells", "tilemap_get_cells", "tilemap_clear",
	"gridmap_set_cells", "gridmap_get_cells", "gridmap_clear",
	"physics_set_layers",
	"signal_connect", "signal_disconnect", "signal_list",
	"node_set_groups", "node_set_script", "node_set_meta", "node_find",
]


static func dispatch(operation: String, params: Dictionary) -> Dictionary:
	## Main entry point. Called by plugin.gd for every incoming operation.
	var scene_path: String = params.get("scene_path", "")

	# For scene-targeting operations, check if the scene is the active tab.
	if scene_path != "" and operation in SCENE_OPS:
		var active_root := _get_active_scene_root(scene_path)
		if active_root != null:
			return _dispatch_live(operation, params, active_root)

	# All other cases: delegate to headless ops + editor sync.
	var result := _dispatch_headless(operation, params)
	_post_operation_sync(operation, params, result)
	return result


# ---------------------------------------------------------------------------
# Active scene detection
# ---------------------------------------------------------------------------

static func _get_active_scene_root(scene_path: String) -> Node:
	## Returns the scene root if scene_path is the currently active editor scene.
	## Returns null otherwise.
	var full_path := "res://" + scene_path
	var root := EditorInterface.get_edited_scene_root()
	if root != null and root.scene_file_path == full_path:
		return root
	return null


# ---------------------------------------------------------------------------
# Live tree operations (active scene)
# ---------------------------------------------------------------------------

static func _dispatch_live(operation: String, params: Dictionary, scene_root: Node) -> Dictionary:
	match operation:
		"scene_read":
			return _live_scene_read(params, scene_root)
		"node_add":
			return _live_node_add(params, scene_root)
		"node_set_properties":
			return _live_node_set_properties(params, scene_root)
		"node_remove":
			return _live_node_remove(params, scene_root)
		"node_reparent":
			return _live_node_reparent(params, scene_root)
		"scene_add_instance":
			return _live_scene_add_instance(params, scene_root)
		"tilemap_set_cells":
			return _live_tilemap_set_cells(params, scene_root)
		"tilemap_get_cells":
			return _live_tilemap_get_cells(params, scene_root)
		"tilemap_clear":
			return _live_tilemap_clear(params, scene_root)
		"gridmap_set_cells":
			return _live_gridmap_set_cells(params, scene_root)
		"gridmap_get_cells":
			return _live_gridmap_get_cells(params, scene_root)
		"gridmap_clear":
			return _live_gridmap_clear(params, scene_root)
		"physics_set_layers":
			return _live_physics_set_layers(params, scene_root)
		"signal_connect":
			return _live_signal_connect(params, scene_root)
		"signal_disconnect":
			return _live_signal_disconnect(params, scene_root)
		"signal_list":
			return _live_signal_list(params, scene_root)
		"node_set_groups":
			return _live_node_set_groups(params, scene_root)
		"node_set_script":
			return _live_node_set_script(params, scene_root)
		"node_set_meta":
			return _live_node_set_meta(params, scene_root)
		"node_find":
			return _live_node_find(params, scene_root)
		_:
			return OpsUtil._error("Unknown live operation: " + operation, operation, params)


static func _live_scene_read(params: Dictionary, scene_root: Node) -> Dictionary:
	## Read the live scene tree (sees unsaved changes).
	var depth: int = params.get("depth", -1)
	var include_props: bool = params.get("properties", true)
	var root_data := SceneOps._read_node(scene_root, 0, depth, include_props)
	return {"success": true, "data": {"root": root_data}}


static func _live_node_add(params: Dictionary, scene_root: Node) -> Dictionary:
	## Add a node to the live scene tree.
	var parent_path: String = params.get("parent_path", "")
	var node_type: String = params.get("node_type", "")
	var node_name: String = params.get("node_name", "")
	var properties: Dictionary = params.get("properties", {})

	if node_type == "":
		return OpsUtil._error("node_type is required", "node_add", params)
	if node_name == "":
		return OpsUtil._error("node_name is required", "node_add", params)
	if not ClassDB.class_exists(node_type):
		return OpsUtil._error("Unknown node type: " + node_type, "node_add", params)
	if not ClassDB.is_parent_class(node_type, "Node"):
		return OpsUtil._error(node_type + " is not a Node subclass", "node_add", params)

	var parent: Node = _resolve_node(scene_root, parent_path)
	if parent == null:
		return OpsUtil._error("Parent node not found: " + parent_path, "node_add", params)

	var node: Node = ClassDB.instantiate(node_type)
	node.name = node_name
	parent.add_child(node)
	node.owner = scene_root

	if not properties.is_empty():
		NodeOps._apply_properties(node, properties)

	var node_path := str(scene_root.get_path_to(node))
	return {"success": true, "data": {"node_path": node_path, "type": node_type}}


static func _live_node_set_properties(params: Dictionary, scene_root: Node) -> Dictionary:
	## Set properties on a node in the live scene tree.
	var node_path: String = params.get("node_path", "")
	var properties: Dictionary = params.get("properties", {})

	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_properties", params)

	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_properties", params)

	var set_props: Array = NodeOps._apply_properties(node, properties)
	return {"success": true, "data": {"node_path": node_path, "properties_set": set_props}}


static func _live_node_remove(params: Dictionary, scene_root: Node) -> Dictionary:
	## Remove a node from the live scene tree.
	var node_path: String = params.get("node_path", "")

	if node_path == "":
		return OpsUtil._error("node_path is required", "node_remove", params)

	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "node_remove", params)
	if node == scene_root:
		return OpsUtil._error("Cannot remove scene root", "node_remove", params)

	var children_count := node.get_child_count()
	node.get_parent().remove_child(node)
	node.queue_free()

	return {"success": true, "data": {"removed": node_path, "children_removed": children_count}}


static func _live_node_reparent(params: Dictionary, scene_root: Node) -> Dictionary:
	## Reparent a node within the live scene tree.
	var node_path: String = params.get("node_path", "")
	var new_parent_path: String = params.get("new_parent_path", "")

	if node_path == "":
		return OpsUtil._error("node_path is required", "node_reparent", params)
	if new_parent_path == "":
		return OpsUtil._error("new_parent_path is required", "node_reparent", params)

	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "node_reparent", params)

	var new_parent: Node = _resolve_node(scene_root, new_parent_path)
	if new_parent == null:
		return OpsUtil._error("New parent not found: " + new_parent_path, "node_reparent", params)

	var old_path := str(scene_root.get_path_to(node))
	node.reparent(new_parent)
	var new_path := str(scene_root.get_path_to(node))

	return {"success": true, "data": {"old_path": old_path, "new_path": new_path}}


static func _live_scene_add_instance(params: Dictionary, scene_root: Node) -> Dictionary:
	## Add a scene instance to the live scene tree.
	var instance_scene: String = params.get("instance_scene", "")
	var parent_path: String = params.get("parent_path", "")
	var node_name: String = params.get("node_name", "")

	if instance_scene == "":
		return OpsUtil._error("instance_scene is required", "scene_add_instance", params)

	var full_scene_path := "res://" + instance_scene
	if not ResourceLoader.exists(full_scene_path):
		return OpsUtil._error("Scene not found: " + instance_scene, "scene_add_instance", params)

	var packed: PackedScene = load(full_scene_path)
	if packed == null:
		return OpsUtil._error("Failed to load scene: " + instance_scene, "scene_add_instance", params)

	var parent: Node = _resolve_node(scene_root, parent_path)
	if parent == null:
		return OpsUtil._error("Parent node not found: " + parent_path, "scene_add_instance", params)

	var instance: Node = packed.instantiate()
	if node_name != "":
		instance.name = node_name
	parent.add_child(instance)
	instance.owner = scene_root

	# Set owner for all children of the instance so they are saved with the scene.
	_set_owner_recursive(instance, scene_root)

	var result_path := str(scene_root.get_path_to(instance))
	return {"success": true, "data": {"node_path": result_path, "instance_scene": instance_scene}}


static func _live_tilemap_set_cells(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "tilemap_set_cells", params)
	return TileMapOps._set_cells_on_node(node, params)


static func _live_tilemap_get_cells(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "tilemap_get_cells", params)
	return TileMapOps._get_cells_from_node(node, params)


static func _live_tilemap_clear(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "tilemap_clear", params)
	return TileMapOps._clear_node(node, params)


static func _live_gridmap_set_cells(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "gridmap_set_cells", params)
	return GridMapOps._set_cells_on_node(node, params)


static func _live_gridmap_get_cells(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "gridmap_get_cells", params)
	return GridMapOps._get_cells_from_node(node, params)


static func _live_gridmap_clear(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "gridmap_clear", params)
	return GridMapOps._clear_node(node, params)


static func _live_signal_connect(params: Dictionary, scene_root: Node) -> Dictionary:
	var source_path: String = params.get("source_path", "")
	var signal_name: String = params.get("signal_name", "")
	var target_path: String = params.get("target_path", "")
	var method_name: String = params.get("method_name", "")
	var flags: int = params.get("flags", 0)

	if source_path == "":
		return OpsUtil._error("source_path is required", "signal_connect", params)
	if signal_name == "":
		return OpsUtil._error("signal_name is required", "signal_connect", params)
	if target_path == "":
		return OpsUtil._error("target_path is required", "signal_connect", params)
	if method_name == "":
		return OpsUtil._error("method_name is required", "signal_connect", params)

	var source: Node = _resolve_node(scene_root, source_path)
	if source == null:
		return OpsUtil._error("Source node not found: " + source_path, "signal_connect", params)

	var target: Node = _resolve_node(scene_root, target_path)
	if target == null:
		return OpsUtil._error("Target node not found: " + target_path, "signal_connect", params)

	# Validate signal exists
	var signal_exists := false
	for sig in source.get_signal_list():
		if sig["name"] == signal_name:
			signal_exists = true
			break
	if not signal_exists:
		return OpsUtil._error(
			"Signal '" + signal_name + "' not found on " + source.get_class(),
			"signal_connect", params)

	# Ensure CONNECT_PERSIST for scene serialization
	flags = flags | 2  # CONNECT_PERSIST = 2

	var callable := Callable(target, method_name)
	var raw_binds = params.get("binds", null)
	if raw_binds != null and raw_binds is Array and not raw_binds.is_empty():
		callable = callable.bindv(raw_binds)

	source.connect(signal_name, callable, flags)

	return {"success": true, "data": {
		"source_path": source_path,
		"signal_name": signal_name,
		"target_path": target_path,
		"method_name": method_name,
	}}


static func _live_signal_disconnect(params: Dictionary, scene_root: Node) -> Dictionary:
	var source_path: String = params.get("source_path", "")
	var signal_name: String = params.get("signal_name", "")
	var target_path: String = params.get("target_path", "")
	var method_name: String = params.get("method_name", "")

	if source_path == "":
		return OpsUtil._error("source_path is required", "signal_disconnect", params)
	if signal_name == "":
		return OpsUtil._error("signal_name is required", "signal_disconnect", params)
	if target_path == "":
		return OpsUtil._error("target_path is required", "signal_disconnect", params)
	if method_name == "":
		return OpsUtil._error("method_name is required", "signal_disconnect", params)

	var source: Node = _resolve_node(scene_root, source_path)
	if source == null:
		return OpsUtil._error("Source node not found: " + source_path, "signal_disconnect", params)

	var target: Node = _resolve_node(scene_root, target_path)
	if target == null:
		return OpsUtil._error("Target node not found: " + target_path, "signal_disconnect", params)

	var callable := Callable(target, method_name)
	if not source.is_connected(signal_name, callable):
		return OpsUtil._error(
			"Connection does not exist: " + source_path + "." + signal_name + " → " + target_path + "." + method_name,
			"signal_disconnect", params)

	source.disconnect(signal_name, callable)

	return {"success": true, "data": {
		"source_path": source_path,
		"signal_name": signal_name,
		"target_path": target_path,
		"method_name": method_name,
	}}


static func _live_signal_list(params: Dictionary, scene_root: Node) -> Dictionary:
	## For live tree: walk the tree and collect connections via get_signal_connection_list.
	var node_path_filter = params.get("node_path", null)
	var connections: Array = []

	_collect_live_connections(scene_root, scene_root, node_path_filter, connections)

	return {"success": true, "data": {"connections": connections}}


static func _collect_live_connections(node: Node, root: Node, filter, results: Array) -> void:
	var node_path: String = str(root.get_path_to(node))
	if node_path == ".":
		node_path = "."

	for sig in node.get_signal_list():
		var sig_name: String = sig["name"]
		for conn in node.get_signal_connection_list(sig_name):
			var callable: Callable = conn["callable"]
			var target = callable.get_object()
			if target == null:
				continue
			var tgt_path: String = str(root.get_path_to(target))
			if tgt_path.begins_with("./"):
				tgt_path = tgt_path.trim_prefix("./")
			if tgt_path == ".":
				tgt_path = "."
			var src_path: String = node_path

			if filter != null and filter != "":
				if src_path != filter and tgt_path != filter:
					continue

			results.append({
				"source_path": src_path,
				"signal_name": sig_name,
				"target_path": tgt_path,
				"method_name": callable.get_method(),
				"flags": conn.get("flags", 0),
			})

	for child in node.get_children():
		_collect_live_connections(child, root, filter, results)


static func _live_node_set_groups(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var add = params.get("add", null)
	var remove = params.get("remove", null)

	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_groups", params)
	if (add == null or (add is Array and add.is_empty())) and \
		(remove == null or (remove is Array and remove.is_empty())):
		return OpsUtil._error("At least one of 'add' or 'remove' must be provided", "node_set_groups", params)

	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_groups", params)

	if add is Array:
		for group in add:
			node.add_to_group(str(group), true)

	if remove is Array:
		for group in remove:
			node.remove_from_group(str(group))

	var final_groups: Array = []
	for group in node.get_groups():
		if not str(group).begins_with("_"):
			final_groups.append(group)

	return {"success": true, "data": {"node_path": node_path, "groups": final_groups}}


static func _live_node_set_script(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var script_path = params.get("script_path", null)

	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_script", params)

	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_script", params)

	var result_script_path = null

	if script_path != null and str(script_path) != "":
		var sp: String = str(script_path)
		if not sp.begins_with("res://"):
			sp = "res://" + sp

		if not ResourceLoader.exists(sp):
			return OpsUtil._error("Script not found: " + sp, "node_set_script", params)

		var script = load(sp)
		if not script is Script:
			return OpsUtil._error("File is not a Script resource: " + sp, "node_set_script", params)

		node.set_script(script)
		result_script_path = sp.replace("res://", "")
	else:
		node.set_script(null)

	return {"success": true, "data": {"node_path": node_path, "script_path": result_script_path}}


static func _live_node_set_meta(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var meta = params.get("meta", null)

	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_meta", params)
	if not meta is Dictionary:
		return OpsUtil._error("meta must be a Dictionary", "node_set_meta", params)

	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_meta", params)

	for key in meta:
		var value = meta[key]
		if value == null:
			if node.has_meta(key):
				node.remove_meta(key)
		else:
			node.set_meta(key, value)

	var meta_keys: Array = node.get_meta_list()
	return {"success": true, "data": {"node_path": node_path, "meta_keys": meta_keys}}


static func _live_node_find(params: Dictionary, scene_root: Node) -> Dictionary:
	var filter_class = params.get("class_name", null)
	var filter_group = params.get("group", null)
	var filter_name_pattern = params.get("name_pattern", null)
	var filter_property = params.get("property", null)
	var filter_property_value = params.get("property_value", null)
	var limit: int = params.get("limit", 100)

	if filter_class == null and filter_group == null and filter_name_pattern == null and filter_property == null:
		return OpsUtil._error(
			"At least one filter must be provided (class_name, group, name_pattern, or property)",
			"node_find", params)

	var results: Array = []
	NodeOps._find_nodes_recursive(scene_root, scene_root, filter_class, filter_group,
		filter_name_pattern, filter_property, filter_property_value, limit, results)

	return {"success": true, "data": {"results": results}}


static func _live_physics_set_layers(params: Dictionary, scene_root: Node) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var collision_layer = params.get("collision_layer", null)
	var collision_mask = params.get("collision_mask", null)

	if node_path == "":
		return OpsUtil._error("node_path is required", "physics_set_layers", params)
	if collision_layer == null and collision_mask == null:
		return OpsUtil._error(
			"At least one of collision_layer or collision_mask is required",
			"physics_set_layers", params)

	var node: Node = _resolve_node(scene_root, node_path)
	if node == null:
		return OpsUtil._error("Node not found: " + node_path,
			"physics_set_layers", params)

	# Verify collision properties exist
	var has_layer := "collision_layer" in node
	var has_mask := "collision_mask" in node
	if not has_layer and not has_mask:
		return OpsUtil._error(
			"Node " + node_path + " (" + node.get_class() +
			") has no collision properties",
			"physics_set_layers",
			{"node_path": node_path, "class": node.get_class()})

	if collision_layer != null:
		node.collision_layer = int(collision_layer)
	if collision_mask != null:
		node.collision_mask = int(collision_mask)

	return {"success": true, "data": {
		"node_path": node_path,
		"collision_layer": node.collision_layer,
		"collision_mask": node.collision_mask,
	}}


# ---------------------------------------------------------------------------
# Headless fallthrough (delegates to ops/ methods)
# ---------------------------------------------------------------------------

static func _dispatch_headless(operation: String, params: Dictionary) -> Dictionary:
	## Same dispatch table as daemon.gd — delegates to regular ops/ methods.
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
		"batch": return MetaOps.op_batch(params)
		"scene_diff": return MetaOps.op_scene_diff(params)
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
		"ping":
			return {"success": true, "data": {"status": "ok", "backend": "editor"}, "operation": "ping"}
		_:
			return OpsUtil._error("Unknown operation: " + operation, operation, {})


# ---------------------------------------------------------------------------
# Post-operation editor sync
# ---------------------------------------------------------------------------

static func _post_operation_sync(operation: String, params: Dictionary, result: Dictionary) -> void:
	## After a headless operation, sync the editor state.
	## - Reload open scenes modified on disk.
	## - Scan filesystem so new/modified files appear in FileSystem dock.
	if not result.get("success", false):
		return

	var scene_path: String = params.get("scene_path", "")
	if scene_path != "":
		var full_path := "res://" + scene_path
		if full_path in EditorInterface.get_open_scenes():
			EditorInterface.reload_scene_from_path(full_path)

	EditorInterface.get_resource_filesystem().scan()


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

static func _resolve_node(scene_root: Node, path: String) -> Node:
	## Resolve a node path relative to the scene root.
	## Empty path or "." returns the scene root itself.
	if path == "" or path == ".":
		return scene_root
	return scene_root.get_node_or_null(NodePath(path))


static func _set_owner_recursive(node: Node, owner: Node) -> void:
	## Set owner for a node and all its descendants (needed for scene serialization).
	for child in node.get_children():
		child.owner = owner
		_set_owner_recursive(child, owner)
