extends SceneTree

## Director headless operations dispatcher.
## Called via: godot --headless --path <project> --script addons/director/operations.gd -- <op> '<json>'

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


func _init():
	var args = _parse_args()
	if args.error != "":
		_print_error(args.error, "parse_args", {})
		quit(1)
		return

	var result = {}
	match args.operation:
		"scene_create":
			result = SceneOps.op_scene_create(args.params)
		"scene_read":
			result = SceneOps.op_scene_read(args.params)
		"node_add":
			result = NodeOps.op_node_add(args.params)
		"node_set_properties":
			result = NodeOps.op_node_set_properties(args.params)
		"node_remove":
			result = NodeOps.op_node_remove(args.params)
		"node_reparent":
			result = NodeOps.op_node_reparent(args.params)
		"scene_list":
			result = SceneOps.op_scene_list(args.params)
		"scene_add_instance":
			result = SceneOps.op_scene_add_instance(args.params)
		"resource_read":
			result = ResourceOps.op_resource_read(args.params)
		"material_create":
			result = ResourceOps.op_material_create(args.params)
		"shape_create":
			result = ResourceOps.op_shape_create(args.params)
		"style_box_create":
			result = ResourceOps.op_style_box_create(args.params)
		"resource_duplicate":
			result = ResourceOps.op_resource_duplicate(args.params)
		"tilemap_set_cells":
			result = TileMapOps.op_tilemap_set_cells(args.params)
		"tilemap_get_cells":
			result = TileMapOps.op_tilemap_get_cells(args.params)
		"tilemap_clear":
			result = TileMapOps.op_tilemap_clear(args.params)
		"gridmap_set_cells":
			result = GridMapOps.op_gridmap_set_cells(args.params)
		"gridmap_get_cells":
			result = GridMapOps.op_gridmap_get_cells(args.params)
		"gridmap_clear":
			result = GridMapOps.op_gridmap_clear(args.params)
		"animation_create":
			result = AnimationOps.op_animation_create(args.params)
		"animation_add_track":
			result = AnimationOps.op_animation_add_track(args.params)
		"animation_read":
			result = AnimationOps.op_animation_read(args.params)
		"animation_remove_track":
			result = AnimationOps.op_animation_remove_track(args.params)
		"physics_set_layers":
			result = PhysicsOps.op_physics_set_layers(args.params)
		"physics_set_layer_names":
			result = PhysicsOps.op_physics_set_layer_names(args.params)
		"visual_shader_create":
			result = ShaderOps.op_visual_shader_create(args.params)
		"batch":
			result = MetaOps.op_batch(args.params)
		"scene_diff":
			result = MetaOps.op_scene_diff(args.params)
		"uid_get":
			result = ProjectOps.op_uid_get(args.params)
		"uid_update_project":
			result = ProjectOps.op_uid_update_project(args.params)
		"export_mesh_library":
			result = ProjectOps.op_export_mesh_library(args.params)
		"signal_connect":
			result = SignalOps.op_signal_connect(args.params)
		"signal_disconnect":
			result = SignalOps.op_signal_disconnect(args.params)
		"signal_list":
			result = SignalOps.op_signal_list(args.params)
		"node_set_groups":
			result = NodeOps.op_node_set_groups(args.params)
		"node_set_script":
			result = NodeOps.op_node_set_script(args.params)
		"node_set_meta":
			result = NodeOps.op_node_set_meta(args.params)
		"node_find":
			result = NodeOps.op_node_find(args.params)
		_:
			result = {"success": false, "error": "Unknown operation: " + args.operation, "operation": args.operation, "context": {}}

	print(JSON.stringify(result))
	quit(0)


func _parse_args() -> Dictionary:
	var cmdline = OS.get_cmdline_user_args()
	if cmdline.size() < 2:
		return {"error": "Usage: operations.gd <operation> '<json_params>'", "operation": "", "params": {}}

	var operation = cmdline[0]
	var json_str = cmdline[1]
	var json = JSON.new()
	var err = json.parse(json_str)
	if err != OK:
		return {"error": "Invalid JSON: " + json.get_error_message(), "operation": operation, "params": {}}

	return {"error": "", "operation": operation, "params": json.get_data()}


func _print_error(message: String, operation: String, context: Dictionary):
	var result = {
		"success": false,
		"error": message,
		"operation": operation,
		"context": context,
	}
	print(JSON.stringify(result))
