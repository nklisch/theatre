# Director Tools — Full Parameter Reference

All tools require `project_path` (string, required) as the first parameter.

## Scene Tools

### scene_create
Create a new .tscn scene file.
- `scene_path` (string, required) — res:// path for the scene
- `root_type` (string, required) — Root node class (Node3D, Node2D, Control, etc.)

### scene_read
Read a scene's node tree.
- `scene_path` (string, required) — Scene to read
- `depth` (int, optional) — Max tree depth
- `properties` (bool, default true) — Include node properties

### scene_list
List scene files in the project.
- `directory` (string, optional) — Subdirectory to list
- `pattern` (string, optional) — Glob filter

### scene_diff
Compare two scenes structurally.
- `scene_a` (string, required) — First scene (supports `HEAD:res://...` git refs)
- `scene_b` (string, required) — Second scene

### scene_add_instance
Add a scene instance as a child.
- `scene_path` (string, required) — Scene to modify
- `instance_scene` (string, required) — Scene to instance
- `parent_path` (string, default ".") — Parent node
- `node_name` (string, optional) — Override instance name

## Node Tools

### node_add
Add a node to a scene.
- `scene_path` (string, required)
- `parent_path` (string, default ".") — Parent node path
- `node_type` (string, required) — Godot class name
- `node_name` (string, required)
- `properties` (object, optional) — Initial properties

### node_remove
Remove a node and all its children.
- `scene_path` (string, required)
- `node_path` (string, required)

### node_set_properties
Set properties on a node. Handles type conversion automatically.
- `scene_path` (string, required)
- `node_path` (string, required)
- `properties` (object, required) — Key-value property map

### node_reparent
Move a node to a new parent.
- `scene_path` (string, required)
- `node_path` (string, required)
- `new_parent_path` (string, required)
- `new_name` (string, optional) — Rename during move

### node_find
Search for nodes in a scene.
- `scene_path` (string, required)
- `class_name` (string, optional) — Filter by class
- `name` (string, optional) — Filter by name pattern
- `group` (string, optional) — Filter by group
- `script` (string, optional) — Filter by script path

### node_set_groups
Add/remove node from groups.
- `scene_path` (string, required)
- `node_path` (string, required)
- `add` (string[], optional) — Groups to add
- `remove` (string[], optional) — Groups to remove

### node_set_script
Attach or detach a GDScript.
- `scene_path` (string, required)
- `node_path` (string, required)
- `script_path` (string, optional) — Script path, or omit to detach

### node_set_meta
Set or remove metadata entries.
- `scene_path` (string, required)
- `node_path` (string, required)
- `meta` (object, required) — Metadata key-value map (null to remove)

## Resource Tools

### resource_read
Read a .tres/.res resource file.
- `resource_path` (string, required)
- `depth` (int, default 1) — Nesting depth

### resource_duplicate
Duplicate a resource with optional overrides.
- `source_path` (string, required)
- `dest_path` (string, required)
- `property_overrides` (object, optional)
- `deep_copy` (bool, default false)

### material_create
Create a material resource.
- `resource_path` (string, required)
- `material_type` (string, required) — StandardMaterial3D, ORMMaterial3D, ShaderMaterial, etc.
- `properties` (object, optional)
- `shader_path` (string, optional) — For ShaderMaterial

### shape_create
Create a collision shape resource.
- `shape_type` (string, required) — BoxShape3D, SphereShape3D, CapsuleShape3D, CircleShape2D, etc.
- `shape_params` (object, optional)
- `save_path` (string, optional) — .tres save path
- `scene_path` (string, optional) — Scene to attach to
- `node_path` (string, optional) — CollisionShape node

### style_box_create
Create a StyleBox resource for UI theming.
- `resource_path` (string, required)
- `style_type` (string, required) — StyleBoxFlat, StyleBoxTexture, StyleBoxLine, StyleBoxEmpty
- `properties` (object, optional)

## TileMap Tools

### tilemap_set_cells
Set cells on a TileMapLayer.
- `scene_path` (string, required)
- `node_path` (string, required) — TileMapLayer node
- `cells` (array, required) — `[{coords: [x,y], source_id, atlas_coords: [x,y], alternative_tile?}]`

### tilemap_get_cells
Read cells from a TileMapLayer.
- `scene_path` (string, required)
- `node_path` (string, required)
- `region` (object, optional) — `{position: [x,y], size: [w,h]}`
- `source_id` (int, optional) — Filter by source

### tilemap_clear
Clear cells from a TileMapLayer.
- `scene_path` (string, required)
- `node_path` (string, required)
- `region` (object, optional) — Region to clear (omit for all)

## GridMap Tools

### gridmap_set_cells
Set 3D grid cells.
- `scene_path` (string, required)
- `node_path` (string, required)
- `cells` (array, required) — `[{position: [x,y,z], item, orientation?}]`

### gridmap_get_cells
Read cells from a GridMap.
- `scene_path` (string, required)
- `node_path` (string, required)
- `bounds` (object, optional) — `{min: [x,y,z], max: [x,y,z]}`
- `item` (int, optional) — Filter by item

### gridmap_clear
Clear cells from a GridMap.
- `scene_path` (string, required)
- `node_path` (string, required)
- `bounds` (object, optional)

## Animation Tools

### animation_create
Create an animation resource.
- `resource_path` (string, required)
- `length` (float, required) — Duration in seconds
- `loop_mode` (string, default "none") — none, linear, pingpong
- `step` (float, optional) — Keyframe snap step

### animation_add_track
Add a track with keyframes.
- `resource_path` (string, required)
- `track_type` (string, required) — value, position_3d, rotation_3d, scale_3d, blend_shape, method, bezier
- `node_path` (string, required) — Node path relative to AnimationPlayer
- `keyframes` (array, required) — `[{time, value, transition?, method?, args?, in_handle?, out_handle?}]`
- `interpolation` (string, default "linear") — nearest, linear, cubic
- `update_mode` (string, default "continuous") — continuous, discrete, capture

### animation_read
Read an animation resource.
- `resource_path` (string, required)

### animation_remove_track
Remove a track from an animation.
- `resource_path` (string, required)
- `track_index` (int, optional) — Remove by index
- `node_path` (string, optional) — Remove all tracks at path

## Physics Tools

### physics_set_layers
Set collision layer/mask bitmasks.
- `scene_path` (string, required)
- `node_path` (string, required)
- `collision_layer` (int, optional)
- `collision_mask` (int, optional)

### physics_set_layer_names
Name physics/render/navigation layers in project.godot.
- `layer_type` (string, required) — 2d_physics, 3d_physics, 2d_render, 3d_render, 2d_navigation, 3d_navigation, avoidance
- `layers` (object, required) — `{layer_number: "name", ...}`

## Signal Tools

### signal_connect
Connect a signal between nodes.
- `scene_path` (string, required)
- `source_path` (string, required)
- `signal_name` (string, required)
- `target_path` (string, required)
- `method_name` (string, required)
- `binds` (array, optional) — Extra arguments
- `flags` (int, optional) — CONNECT_DEFERRED, PERSIST, ONE_SHOT

### signal_disconnect
Remove a signal connection.
- `scene_path` (string, required)
- `source_path` (string, required)
- `signal_name` (string, required)
- `target_path` (string, required)
- `method_name` (string, required)

### signal_list
List signal connections in a scene.
- `scene_path` (string, required)
- `node_path` (string, optional) — Filter by node

## Other Tools

### visual_shader_create
Create a VisualShader with node graph.
- `resource_path` (string, required)
- `shader_mode` (string, required) — spatial, canvas_item, particles, sky, fog
- `nodes` (array, required) — `[{node_id, type, shader_function?, position, properties?}]`
- `connections` (array, optional) — `[{from_node, from_port, to_node, to_port, shader_function?}]`

### export_mesh_library
Export MeshInstance3D nodes as MeshLibrary.
- `scene_path` (string, required)
- `output_path` (string, required)
- `items` (string[], optional) — Node names to include

### uid_get
Resolve a file's Godot UID.
- `file_path` (string, required)

### uid_update_project
Scan and register missing UIDs.
- `directory` (string, optional) — Subdirectory to scan

### batch
Execute multiple operations in one Godot invocation.
- `operations` (array, required) — `[{operation: "tool_name", params: {...}}]`
- `stop_on_error` (bool, default true)
