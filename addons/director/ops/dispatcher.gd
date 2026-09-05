@tool

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
const EngineApiOps = preload("res://addons/director/ops/engine_api_ops.gd")
const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")

static func dispatch(operation: String, params: Dictionary, borrowed: Node = null, manager: EditorUndoRedoManager = null) -> Dictionary:
	var edit: SceneEdit = null
	if operation in SceneEdit.OPERATIONS and (operation != "shape_create" or params.get("scene_path", "") != ""):
		edit = SceneEdit.new()
		var opened: Dictionary = edit.open(operation, params, borrowed, manager)
		if not opened.success:
			return opened
	var result: Dictionary
	if operation == "scene_save":
		result = SceneEdit.save_root(edit.root, edit.path)
		if result.success:
			result["data"] = {"scene_path": edit.path, "editor_dirty_marker_may_remain": not edit.owned}
			result["persistence"] = {"saved_paths": [edit.path], "unsaved_scene_paths": []}
	else:
		result = _run(operation, params, edit)
	if edit != null:
		result = edit.finish(result)
	if not result.get("success", false):
		result["operation"] = operation
		var context := params.duplicate()
		context.merge(result.get("context", {}), true)
		result["context"] = context
	return file_persistence(operation, params, result)

static func file_persistence(operation: String, params: Dictionary, result: Dictionary) -> Dictionary:
	var persistence: Dictionary = result.get("persistence", {"saved_paths": [], "unsaved_scene_paths": []})
	if result.get("success", false):
		var field: String = {"scene_create": "scene_path", "material_create": "resource_path", "style_box_create": "resource_path", "resource_duplicate": "dest_path", "animation_create": "resource_path", "animation_add_track": "resource_path", "animation_remove_track": "resource_path", "visual_shader_create": "resource_path", "export_mesh_library": "output_path", "shape_create": "save_path"}.get(operation, "")
		if field != "" and params.get(field, "") != "" and not params[field] in persistence.saved_paths:
			persistence.saved_paths.append(str(params[field]).trim_prefix("res://"))
		if operation in ["autoload_add", "autoload_remove", "project_settings_set", "physics_set_layer_names"]:
			persistence.saved_paths.append("project.godot")
	result["persistence"] = persistence
	if result.get("data") is Dictionary:
		result.data["persistence"] = persistence
	return result

static func _run(operation: String, params: Dictionary, edit: SceneEdit) -> Dictionary:
	match operation:
		"scene_create":
			return SceneOps.op_scene_create(params)
		"scene_read":
			return SceneOps.op_scene_read(params, edit)
		"node_add":
			return NodeOps.op_node_add(params, edit)
		"node_set_properties":
			return NodeOps.op_node_set_properties(params, edit)
		"node_remove":
			return NodeOps.op_node_remove(params, edit)
		"node_reparent":
			return NodeOps.op_node_reparent(params, edit)
		"scene_list":
			return SceneOps.op_scene_list(params)
		"scene_add_instance":
			return SceneOps.op_scene_add_instance(params, edit)
		"resource_read":
			return ResourceOps.op_resource_read(params)
		"material_create":
			return ResourceOps.op_material_create(params)
		"shape_create":
			return ResourceOps.op_shape_create(params, edit)
		"style_box_create":
			return ResourceOps.op_style_box_create(params)
		"resource_duplicate":
			return ResourceOps.op_resource_duplicate(params)
		"tilemap_set_cells":
			return TileMapOps.op_tilemap_set_cells(params, edit)
		"tilemap_get_cells":
			return TileMapOps.op_tilemap_get_cells(params, edit)
		"tilemap_clear":
			return TileMapOps.op_tilemap_clear(params, edit)
		"gridmap_set_cells":
			return GridMapOps.op_gridmap_set_cells(params, edit)
		"gridmap_get_cells":
			return GridMapOps.op_gridmap_get_cells(params, edit)
		"gridmap_clear":
			return GridMapOps.op_gridmap_clear(params, edit)
		"animation_create":
			return AnimationOps.op_animation_create(params)
		"animation_add_track":
			return AnimationOps.op_animation_add_track(params)
		"animation_read":
			return AnimationOps.op_animation_read(params)
		"animation_remove_track":
			return AnimationOps.op_animation_remove_track(params)
		"physics_set_layers":
			return PhysicsOps.op_physics_set_layers(params, edit)
		"physics_set_layer_names":
			return PhysicsOps.op_physics_set_layer_names(params)
		"visual_shader_create":
			return ShaderOps.op_visual_shader_create(params)
		"batch":
			return MetaOps.op_batch(params, dispatch)
		"scene_diff":
			return MetaOps.op_scene_diff(params)
		"autoload_add":
			return ProjectOps.op_autoload_add(params)
		"autoload_remove":
			return ProjectOps.op_autoload_remove(params)
		"project_settings_set":
			return ProjectOps.op_project_settings_set(params)
		"project_reload":
			return ProjectOps.op_project_reload(params)
		"editor_status":
			return ProjectOps.op_editor_status(params)
		"uid_get":
			return ProjectOps.op_uid_get(params)
		"uid_update_project":
			return ProjectOps.op_uid_update_project(params)
		"export_mesh_library":
			return ProjectOps.op_export_mesh_library(params)
		"signal_connect":
			return SignalOps.op_signal_connect(params, edit)
		"signal_disconnect":
			return SignalOps.op_signal_disconnect(params, edit)
		"signal_list":
			return SignalOps.op_signal_list(params, edit)
		"node_set_groups":
			return NodeOps.op_node_set_groups(params, edit)
		"node_set_script":
			return NodeOps.op_node_set_script(params, edit)
		"node_set_meta":
			return NodeOps.op_node_set_meta(params, edit)
		"node_find":
			return NodeOps.op_node_find(params, edit)
		"engine_api":
			return EngineApiOps.op_engine_api(params)
		_:
			return {"success": false, "error": "Unknown operation: " + operation, "operation": operation, "context": {}}

