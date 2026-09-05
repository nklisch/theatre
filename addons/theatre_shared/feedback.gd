@tool
extends RefCounted
## Shared support payload, not an editor plugin. Capture and publication are
## synchronous so a composer always refers to its own copied context and image.

const ROOT := "res://.theatre/feedback"
const MAX_ITEMS := 64
const MAX_STORAGE_BYTES := 128 * 1024 * 1024
const MAX_IMAGE_BYTES := 8 * 1024 * 1024
const MAX_NOTE_BYTES := 16 * 1024
const MAX_METADATA_BYTES := 64 * 1024
const MAX_EDGE := 1280

static func capture(viewport: Viewport, source: String, scene: String,
		selection: Array, surface: String, run_id: Variant = null) -> Dictionary:
	var item := {
		"feedback_id": "feedback_%d_%d_%s" % [OS.get_process_id(), Time.get_ticks_usec(), Crypto.new().generate_random_bytes(8).hex_encode()],
		"source": source, "timestamp_ms": int(Time.get_unix_time_from_system() * 1000),
		"project_path": ProjectSettings.globalize_path("res://"),
		"process_id": OS.get_process_id(), "run_id": run_id,
		"scene": scene, "surface": surface, "selection": selection.duplicate(true),
		"pointer": {"status": "unavailable"},
		"capture": {"status": "unavailable", "reason": "Scene viewport unavailable"},
		"readback_render_frame": Engine.get_frames_drawn(),
		"readback_physics_frame": Engine.get_physics_frames(), "note": "",
	}
	var image: Image = null
	if viewport != null:
		var pointer := viewport.get_mouse_position()
		var pointer_rect := viewport.get_visible_rect()
		if DisplayServer.get_name() != "headless":
			if pointer_rect.has_point(pointer):
				item.pointer = {"status": "inside", "position": [pointer.x, pointer.y]}
			else:
				item.pointer = {"status": "outside"}
		if DisplayServer.get_name() == "headless":
			item.capture.reason = "Headless display has no rendered pixels"
		else:
			var texture := viewport.get_texture()
			if texture != null:
				image = texture.get_image()
			if image == null or image.is_empty():
				item.capture.reason = "No completed viewport pixels"
				image = null
			else:
				var dimensions := image.get_size()
				if item.pointer.status == "inside" and pointer_rect.size.x > 0 and pointer_rect.size.y > 0:
					# Root canvas stretching can make logical input coordinates differ
					# from source pixels. Store source-pixel coordinates, before resize.
					var source_pointer := (pointer - pointer_rect.position) * Vector2(dimensions) / pointer_rect.size
					item.pointer.position = [source_pointer.x, source_pointer.y]
				var factor := minf(1.0, float(MAX_EDGE) / maxf(dimensions.x, dimensions.y))
				if factor < 1.0:
					image.resize(maxi(1, roundi(dimensions.x * factor)), maxi(1, roundi(dimensions.y * factor)), Image.INTERPOLATE_BILINEAR)
				item.capture = {"status": "available",
					"source_dimensions": {"width": dimensions.x, "height": dimensions.y},
					"output_dimensions": {"width": image.get_width(), "height": image.get_height()}}
	return {"item": item, "image": image}


static func publish(composition: Dictionary, note: String) -> Dictionary:
	if note.to_utf8_buffer().size() > MAX_NOTE_BYTES:
		return {"error": "Note exceeds 16 KiB. Shorten it before queuing; your composition is retained."}
	var item: Dictionary = composition.item.duplicate(true)
	item.note = note
	var image: Image = composition.image
	var jpeg := PackedByteArray()
	if image != null:
		jpeg = image.save_jpg_to_buffer(0.85)
		if jpeg.is_empty() or jpeg.size() > MAX_IMAGE_BYTES:
			return {"error": "Image encoding failed or exceeds 8 MiB; your composition is retained."}
	var metadata := JSON.stringify(item).to_utf8_buffer()
	if metadata.size() > MAX_METADATA_BYTES:
		return {"error": "Selection/context exceeds 64 KiB; choose a smaller selection and capture again."}
	var root := ProjectSettings.globalize_path(ROOT)
	var err := DirAccess.make_dir_recursive_absolute(root)
	if err != OK:
		return {"error": "Cannot create feedback storage: %s" % error_string(err)}
	var dir := DirAccess.open(root)
	if dir == null:
		return {"error": "Cannot read feedback storage"}
	dir.include_hidden = true
	var count := 0
	for name in dir.get_directories():
		if name != "handled":
			count += 1
	var used := _storage_bytes(root)
	if used < 0:
		return {"error": "Cannot measure feedback storage; check directory permissions. Your composition is retained."}
	if count >= MAX_ITEMS or used + metadata.size() + jpeg.size() > MAX_STORAGE_BYTES:
		return {"error": "Feedback storage is full (64 items / 128 MiB). Use feedback status then explicitly delete items or cleanup incomplete directories. Existing evidence and this composition are retained."}
	# No lock service: simultaneous producers may slightly exceed admission bounds.
	var pending := root.path_join(".pending-" + str(item.feedback_id))
	err = DirAccess.make_dir_absolute(pending)
	if err != OK:
		return {"error": "Cannot begin feedback publication: %s" % error_string(err)}
	if not jpeg.is_empty():
		err = _write(pending.path_join("image.jpg"), jpeg)
	if err == OK:
		err = _write(pending.path_join("item.json"), metadata)
	if err == OK:
		err = DirAccess.rename_absolute(pending, root.path_join(item.feedback_id))
	if err != OK:
		return {"error": "Publication failed: %s. Composition retained; feedback status exposes incomplete storage for explicit cleanup." % error_string(err)}
	return {"feedback_id": item.feedback_id}


static func _write(path: String, bytes: PackedByteArray) -> Error:
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		return FileAccess.get_open_error()
	file.store_buffer(bytes)
	file.flush()
	var err := file.get_error()
	file.close()
	return err


static func _storage_bytes(path: String) -> int:
	var dir := DirAccess.open(path)
	if dir == null:
		return -1
	dir.include_hidden = true
	var total := 0
	for name in dir.get_files():
		var file := FileAccess.open(path.path_join(name), FileAccess.READ)
		if file == null:
			return -1
		total += file.get_length()
	for name in dir.get_directories():
		# Do not follow links into unrelated project storage during admission.
		if dir.is_link(name):
			continue
		var bytes := _storage_bytes(path.path_join(name))
		if bytes < 0:
			return -1
		total += bytes
	return total
