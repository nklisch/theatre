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
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")

## Scene-targeting operations that can use live tree manipulation.
const SCENE_OPS := [
	"scene_read", "node_add", "node_set_properties", "node_remove",
	"node_reparent", "scene_add_instance",
	"tilemap_set_cells", "tilemap_get_cells", "tilemap_clear",
	"gridmap_set_cells", "gridmap_get_cells", "gridmap_clear",
	"physics_set_layers",
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
