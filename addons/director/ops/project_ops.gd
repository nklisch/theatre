class_name ProjectOps

const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_uid_get(params: Dictionary) -> Dictionary:
	## Resolve the Godot UID for a file path.
	##
	## Params: file_path (String) — relative to project
	## Returns: { success, data: { file_path, uid } }
	var file_path: String = params.get("file_path", "")
	if file_path == "":
		return OpsUtil._error("file_path is required", "uid_get", params)

	var full_path = "res://" + file_path
	if not ResourceLoader.exists(full_path) and not FileAccess.file_exists(full_path):
		return OpsUtil._error("File not found: " + file_path, "uid_get",
			{"file_path": file_path})

	var uid_int: int = ResourceLoader.get_resource_uid(full_path)
	if uid_int == -1:
		return OpsUtil._error("No UID found for: " + file_path, "uid_get",
			{"file_path": file_path})

	var uid_str: String = ResourceUID.id_to_text(uid_int)

	return {"success": true, "data": {
		"file_path": file_path,
		"uid": uid_str,
	}}


static func op_uid_update_project(params: Dictionary) -> Dictionary:
	## Scan project files and register any missing UIDs.
	##
	## Params: directory (String, optional — default "")
	## Returns: { success, data: { files_scanned, uids_registered } }
	var directory: String = params.get("directory", "")
	var base_path: String = "res://" + directory if directory != "" else "res://"

	var files: Array = []
	_collect_resource_files(base_path, files)

	var scanned: int = 0
	var registered: int = 0

	for file_path in files:
		scanned += 1
		var uid_int: int = ResourceLoader.get_resource_uid(file_path)
		if uid_int == -1:
			var new_uid: int = ResourceUID.create_id()
			ResourceUID.set_id(new_uid, file_path)
			registered += 1

	return {"success": true, "data": {
		"files_scanned": scanned,
		"uids_registered": registered,
	}}


static func op_export_mesh_library(params: Dictionary) -> Dictionary:
	## Export MeshInstance3D nodes from a scene as a MeshLibrary resource.
	##
	## Params:
	##   scene_path (String) — source scene
	##   output_path (String) — save path for .tres
	##   items (Array[String], optional) — node names to include; all if omitted
	## Returns: { success, data: { path, items_exported } }
	var scene_path: String = params.get("scene_path", "")
	var output_path: String = params.get("output_path", "")
	var items_filter: Array = params.get("items", [])

	if scene_path == "":
		return OpsUtil._error("scene_path is required",
			"export_mesh_library", params)
	if output_path == "":
		return OpsUtil._error("output_path is required",
			"export_mesh_library", params)

	var full_scene = "res://" + scene_path
	if not ResourceLoader.exists(full_scene):
		return OpsUtil._error("Scene not found: " + scene_path,
			"export_mesh_library", {"scene_path": scene_path})

	var packed: PackedScene = load(full_scene)
	var root = packed.instantiate()

	var mesh_lib = MeshLibrary.new()
	var items_exported: int = 0
	var item_id: int = 0

	for child in root.get_children():
		if not child is MeshInstance3D:
			continue
		if items_filter.size() > 0 and str(child.name) not in items_filter:
			continue

		var mesh_instance: MeshInstance3D = child
		if mesh_instance.mesh == null:
			continue

		mesh_lib.create_item(item_id)
		mesh_lib.set_item_mesh(item_id, mesh_instance.mesh)
		mesh_lib.set_item_name(item_id, str(child.name))

		# Check for a CollisionShape3D child → extract shape for navigation
		for grandchild in child.get_children():
			if grandchild is CollisionShape3D and grandchild.shape != null:
				var shapes: Array = []
				shapes.append(grandchild.shape)
				var transforms: Array = []
				transforms.append(grandchild.transform)
				mesh_lib.set_item_shapes(item_id, shapes + transforms)
				break

		item_id += 1
		items_exported += 1

	root.free()

	if items_exported == 0:
		return OpsUtil._error("No MeshInstance3D nodes found in scene",
			"export_mesh_library",
			{"scene_path": scene_path, "filter": items_filter})

	var full_output = "res://" + output_path
	var dir_path = full_output.get_base_dir()
	if not DirAccess.dir_exists_absolute(dir_path):
		DirAccess.make_dir_recursive_absolute(dir_path)

	var err = ResourceSaver.save(mesh_lib, full_output)
	if err != OK:
		return OpsUtil._error("Failed to save MeshLibrary: " + str(err),
			"export_mesh_library", {"output_path": output_path})

	return {"success": true, "data": {
		"path": output_path,
		"items_exported": items_exported,
	}}


static func _collect_resource_files(dir_path: String, result: Array) -> void:
	## Recursively collect resource files that should have UIDs.
	var dir = DirAccess.open(dir_path)
	if dir == null:
		return
	dir.list_dir_begin()
	var file_name = dir.get_next()
	while file_name != "":
		if file_name != "." and file_name != ".." \
				and not file_name.begins_with("."):
			var full = dir_path.trim_suffix("/") + "/" + file_name
			if dir.current_is_dir():
				if file_name != "addons" or dir_path == "res://":
					_collect_resource_files(full, result)
			else:
				var ext = file_name.get_extension()
				if ext in ["tscn", "tres", "gd", "gdshader"]:
					result.append(full)
		file_name = dir.get_next()
	dir.list_dir_end()
