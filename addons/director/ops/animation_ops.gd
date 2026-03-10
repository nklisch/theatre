class_name AnimationOps

const OpsUtil = preload("res://addons/director/ops/ops_util.gd")
const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")


# ---- Track type mapping ----

const TRACK_TYPES := {
	"value": Animation.TYPE_VALUE,
	"position_3d": Animation.TYPE_POSITION_3D,
	"rotation_3d": Animation.TYPE_ROTATION_3D,
	"scale_3d": Animation.TYPE_SCALE_3D,
	"blend_shape": Animation.TYPE_BLEND_SHAPE,
	"method": Animation.TYPE_METHOD,
	"bezier": Animation.TYPE_BEZIER,
}

static func _get_track_type_name(godot_type: int) -> String:
	for name in TRACK_TYPES:
		if TRACK_TYPES[name] == godot_type:
			return name
	return "unknown_%d" % godot_type


const LOOP_MODES := {
	"none": Animation.LOOP_NONE,
	"linear": Animation.LOOP_LINEAR,
	"pingpong": Animation.LOOP_PINGPONG,
}

const LOOP_MODE_NAMES := {
	Animation.LOOP_NONE: "none",
	Animation.LOOP_LINEAR: "linear",
	Animation.LOOP_PINGPONG: "pingpong",
}

const INTERPOLATION_TYPES := {
	"nearest": Animation.INTERPOLATION_NEAREST,
	"linear": Animation.INTERPOLATION_LINEAR,
	"cubic": Animation.INTERPOLATION_CUBIC,
}

const UPDATE_MODES := {
	"continuous": Animation.UPDATE_CONTINUOUS,
	"discrete": Animation.UPDATE_DISCRETE,
	"capture": Animation.UPDATE_CAPTURE,
}


# ---- Operations ----

static func op_animation_create(params: Dictionary) -> Dictionary:
	## Create an Animation resource and save to disk.
	##
	## Params: resource_path, length, loop_mode?, step?
	## Returns: { success, data: { path, length, loop_mode } }

	var resource_path: String = params.get("resource_path", "")
	var length = params.get("length", null)

	if resource_path == "":
		return OpsUtil._error("resource_path is required", "animation_create", params)
	if length == null:
		return OpsUtil._error("length is required", "animation_create", params)
	if float(length) <= 0.0:
		return OpsUtil._error("length must be positive", "animation_create",
			{"resource_path": resource_path, "length": length})

	var anim := Animation.new()
	anim.length = float(length)

	# Loop mode
	var loop_mode_str: String = params.get("loop_mode", "none")
	if not LOOP_MODES.has(loop_mode_str):
		return OpsUtil._error(
			"Invalid loop_mode: " + loop_mode_str +
			". Must be one of: " + ", ".join(LOOP_MODES.keys()),
			"animation_create", {"loop_mode": loop_mode_str})
	anim.loop_mode = LOOP_MODES[loop_mode_str]

	# Step
	var step = params.get("step", null)
	if step != null:
		anim.step = float(step)

	# Save
	var full_path = "res://" + resource_path
	ResourceOps._ensure_directory(full_path)
	var err = ResourceSaver.save(anim, full_path)
	if err != OK:
		return OpsUtil._error("Failed to save animation: " + str(err),
			"animation_create", {"resource_path": resource_path})

	return {"success": true, "data": {
		"path": resource_path,
		"length": anim.length,
		"loop_mode": loop_mode_str,
	}}


static func op_animation_add_track(params: Dictionary) -> Dictionary:
	## Add a track with keyframes to an existing Animation resource.
	##
	## Params: resource_path, track_type, node_path, keyframes,
	##         interpolation?, update_mode?
	## Returns: { success, data: { track_index, keyframe_count } }

	var resource_path: String = params.get("resource_path", "")
	var track_type_str: String = params.get("track_type", "")
	var node_path_str: String = params.get("node_path", "")
	var keyframes = params.get("keyframes", null)

	if resource_path == "":
		return OpsUtil._error("resource_path is required", "animation_add_track", params)
	if track_type_str == "":
		return OpsUtil._error("track_type is required", "animation_add_track", params)
	if node_path_str == "":
		return OpsUtil._error("node_path is required", "animation_add_track", params)
	if keyframes == null or not keyframes is Array or keyframes.is_empty():
		return OpsUtil._error("keyframes must be a non-empty array",
			"animation_add_track", {"resource_path": resource_path})

	if not TRACK_TYPES.has(track_type_str):
		return OpsUtil._error(
			"Invalid track_type: " + track_type_str +
			". Must be one of: " + ", ".join(TRACK_TYPES.keys()),
			"animation_add_track", {"track_type": track_type_str})

	var full_path = "res://" + resource_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Animation not found: " + resource_path,
			"animation_add_track", {"resource_path": resource_path})

	var loaded = load(full_path)
	if loaded == null or not loaded is Animation:
		return OpsUtil._error("Resource is not an Animation: " + resource_path,
			"animation_add_track", {"resource_path": resource_path})
	var anim: Animation = loaded

	var track_type: int = TRACK_TYPES[track_type_str]
	var track_idx: int = anim.add_track(track_type)
	anim.track_set_path(track_idx, NodePath(node_path_str))

	# Set interpolation
	var interp_str: String = params.get("interpolation", "linear")
	if not INTERPOLATION_TYPES.has(interp_str):
		return OpsUtil._error(
			"Invalid interpolation: " + interp_str +
			". Must be one of: " + ", ".join(INTERPOLATION_TYPES.keys()),
			"animation_add_track", {"interpolation": interp_str})
	anim.track_set_interpolation_type(track_idx, INTERPOLATION_TYPES[interp_str])

	# Set update mode (value tracks only)
	if track_type == Animation.TYPE_VALUE:
		var update_str: String = params.get("update_mode", "continuous")
		if not UPDATE_MODES.has(update_str):
			return OpsUtil._error(
				"Invalid update_mode: " + update_str +
				". Must be one of: " + ", ".join(UPDATE_MODES.keys()),
				"animation_add_track", {"update_mode": update_str})
		anim.value_track_set_update_mode(track_idx, UPDATE_MODES[update_str])

	# Insert keyframes
	var key_count := 0
	for kf in keyframes:
		var result = _insert_keyframe(anim, track_idx, track_type, kf)
		if not result.success:
			return result
		key_count += 1

	# Save
	var err = ResourceSaver.save(anim, full_path)
	if err != OK:
		return OpsUtil._error("Failed to save animation: " + str(err),
			"animation_add_track", {"resource_path": resource_path})

	return {"success": true, "data": {
		"track_index": track_idx,
		"keyframe_count": key_count,
	}}


static func op_animation_read(params: Dictionary) -> Dictionary:
	## Read an Animation resource and serialize all tracks and keyframes.
	##
	## Params: resource_path
	## Returns: { success, data: { path, length, loop_mode, step, tracks: [...] } }

	var resource_path: String = params.get("resource_path", "")
	if resource_path == "":
		return OpsUtil._error("resource_path is required", "animation_read", params)

	var full_path = "res://" + resource_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Animation not found: " + resource_path,
			"animation_read", {"resource_path": resource_path})

	var loaded = load(full_path)
	if loaded == null or not loaded is Animation:
		return OpsUtil._error("Resource is not an Animation: " + resource_path,
			"animation_read", {"resource_path": resource_path})
	var anim: Animation = loaded

	var tracks: Array = []
	for i in range(anim.get_track_count()):
		tracks.append(_serialize_track(anim, i))

	return {"success": true, "data": {
		"path": resource_path,
		"length": anim.length,
		"loop_mode": LOOP_MODE_NAMES.get(anim.loop_mode, "none"),
		"step": anim.step,
		"tracks": tracks,
	}}


static func op_animation_remove_track(params: Dictionary) -> Dictionary:
	## Remove a track from an Animation resource.
	##
	## Params: resource_path, track_index? | node_path?
	## Returns: { success, data: { tracks_removed } }

	var resource_path: String = params.get("resource_path", "")
	var track_index = params.get("track_index", null)
	var node_path_str = params.get("node_path", null)

	if resource_path == "":
		return OpsUtil._error("resource_path is required", "animation_remove_track", params)
	if track_index == null and (node_path_str == null or node_path_str == ""):
		return OpsUtil._error("Either track_index or node_path is required",
			"animation_remove_track", params)
	if track_index != null and node_path_str != null and node_path_str != "":
		return OpsUtil._error("Specify track_index or node_path, not both",
			"animation_remove_track", params)

	var full_path = "res://" + resource_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Animation not found: " + resource_path,
			"animation_remove_track", {"resource_path": resource_path})

	var loaded = load(full_path)
	if loaded == null or not loaded is Animation:
		return OpsUtil._error("Resource is not an Animation: " + resource_path,
			"animation_remove_track", {"resource_path": resource_path})
	var anim: Animation = loaded

	var removed := 0

	if track_index != null:
		var idx := int(track_index)
		if idx < 0 or idx >= anim.get_track_count():
			return OpsUtil._error(
				"Track index out of range: %d (animation has %d tracks)" % [idx, anim.get_track_count()],
				"animation_remove_track",
				{"track_index": idx, "track_count": anim.get_track_count()})
		anim.remove_track(idx)
		removed = 1
	else:
		# Remove all tracks matching node_path (iterate in reverse to
		# avoid index shift issues).
		var target_path := NodePath(node_path_str)
		for i in range(anim.get_track_count() - 1, -1, -1):
			if anim.track_get_path(i) == target_path:
				anim.remove_track(i)
				removed += 1
		if removed == 0:
			return OpsUtil._error(
				"No tracks found with node_path: " + node_path_str,
				"animation_remove_track",
				{"resource_path": resource_path, "node_path": node_path_str})

	# Save
	var err = ResourceSaver.save(anim, full_path)
	if err != OK:
		return OpsUtil._error("Failed to save animation: " + str(err),
			"animation_remove_track", {"resource_path": resource_path})

	return {"success": true, "data": {"tracks_removed": removed}}


# ---- Keyframe insertion per track type ----

static func _insert_keyframe(anim: Animation, track_idx: int,
		track_type: int, kf: Dictionary) -> Dictionary:
	## Insert a single keyframe into a track. Format depends on track type.

	var time: float = kf.get("time", 0.0)

	match track_type:
		Animation.TYPE_VALUE:
			var value = kf.get("value", null)
			if value == null:
				return OpsUtil._error("Keyframe missing 'value'",
					"animation_add_track", {"time": time})
			value = _convert_keyframe_value(value)
			var transition: float = kf.get("transition", 1.0)
			anim.track_insert_key(track_idx, time, value, transition)

		Animation.TYPE_POSITION_3D:
			var v = kf.get("value", {})
			if not v is Dictionary:
				return OpsUtil._error("position_3d keyframe value must be {x, y, z}",
					"animation_add_track", {"time": time})
			var pos := Vector3(v.get("x", 0), v.get("y", 0), v.get("z", 0))
			anim.position_track_insert_key(track_idx, time, pos)

		Animation.TYPE_ROTATION_3D:
			var v = kf.get("value", {})
			if not v is Dictionary:
				return OpsUtil._error("rotation_3d keyframe value must be {x, y, z, w}",
					"animation_add_track", {"time": time})
			var quat := Quaternion(v.get("x", 0), v.get("y", 0),
				v.get("z", 0), v.get("w", 1))
			anim.rotation_track_insert_key(track_idx, time, quat)

		Animation.TYPE_SCALE_3D:
			var v = kf.get("value", {})
			if not v is Dictionary:
				return OpsUtil._error("scale_3d keyframe value must be {x, y, z}",
					"animation_add_track", {"time": time})
			var scale := Vector3(v.get("x", 1), v.get("y", 1), v.get("z", 1))
			anim.scale_track_insert_key(track_idx, time, scale)

		Animation.TYPE_BLEND_SHAPE:
			var value = kf.get("value", null)
			if value == null:
				return OpsUtil._error("blend_shape keyframe missing 'value'",
					"animation_add_track", {"time": time})
			anim.blend_shape_track_insert_key(track_idx, time, float(value))

		Animation.TYPE_METHOD:
			var method_name: String = kf.get("method", "")
			if method_name == "":
				return OpsUtil._error("Method keyframe missing 'method' name",
					"animation_add_track", {"time": time})
			var args_arr = kf.get("args", [])
			if not args_arr is Array:
				args_arr = []
			# Method track keys are dictionaries with "method" and "args"
			anim.track_insert_key(track_idx, time, {
				"method": method_name,
				"args": args_arr,
			})

		Animation.TYPE_BEZIER:
			var value = kf.get("value", null)
			if value == null:
				return OpsUtil._error("bezier keyframe missing 'value'",
					"animation_add_track", {"time": time})
			var in_h = kf.get("in_handle", {"x": 0, "y": 0})
			var out_h = kf.get("out_handle", {"x": 0, "y": 0})
			var in_vec := Vector2(in_h.get("x", 0), in_h.get("y", 0))
			var out_vec := Vector2(out_h.get("x", 0), out_h.get("y", 0))
			anim.bezier_track_insert_key(track_idx, time, float(value),
				in_vec, out_vec)

		_:
			return OpsUtil._error("Unsupported track type for keyframe insertion",
				"animation_add_track", {"track_type": track_type})

	return {"success": true}


static func _convert_keyframe_value(value):
	## Best-effort conversion of JSON keyframe values to Godot types.
	## For value tracks, we don't have the target node's property list,
	## so we use heuristics based on the JSON structure.
	if value is Dictionary:
		if value.has("x") and value.has("y"):
			if value.has("z"):
				if value.has("w"):
					return Quaternion(value.x, value.y, value.z, value.w)
				return Vector3(value.x, value.y, value.z)
			return Vector2(value.x, value.y)
		if value.has("r") and value.has("g") and value.has("b"):
			return Color(value.r, value.g, value.b, value.get("a", 1.0))
	if value is String and value.begins_with("#"):
		return Color.html(value)
	return value


# ---- Track serialization ----

static func _serialize_track(anim: Animation, track_idx: int) -> Dictionary:
	## Serialize a single track with all its keyframes.
	var track_type := anim.track_get_type(track_idx)
	var type_name := _get_track_type_name(track_type)
	var node_path := str(anim.track_get_path(track_idx))

	var track_data := {
		"track_index": track_idx,
		"type": type_name,
		"node_path": node_path,
		"interpolation": _get_interpolation_name(
			anim.track_get_interpolation_type(track_idx)),
	}

	# Value tracks have update mode
	if track_type == Animation.TYPE_VALUE:
		track_data["update_mode"] = _get_update_mode_name(
			anim.value_track_get_update_mode(track_idx))

	# Serialize keyframes
	var keyframes: Array = []
	var key_count := anim.track_get_key_count(track_idx)
	for k in range(key_count):
		keyframes.append(_serialize_keyframe(anim, track_idx, track_type, k))

	track_data["keyframes"] = keyframes
	return track_data


static func _serialize_keyframe(anim: Animation, track_idx: int,
		track_type: int, key_idx: int) -> Dictionary:
	## Serialize a single keyframe, format depends on track type.
	var kf := {"time": anim.track_get_key_time(track_idx, key_idx)}

	match track_type:
		Animation.TYPE_VALUE:
			kf["value"] = SceneOps._serialize_value(
				anim.track_get_key_value(track_idx, key_idx))
			var transition := anim.track_get_key_transition(track_idx, key_idx)
			if transition != 1.0:
				kf["transition"] = transition

		Animation.TYPE_POSITION_3D, Animation.TYPE_SCALE_3D:
			var v: Vector3 = anim.track_get_key_value(track_idx, key_idx)
			kf["value"] = {"x": v.x, "y": v.y, "z": v.z}

		Animation.TYPE_ROTATION_3D:
			var q: Quaternion = anim.track_get_key_value(track_idx, key_idx)
			kf["value"] = {"x": q.x, "y": q.y, "z": q.z, "w": q.w}

		Animation.TYPE_BLEND_SHAPE:
			kf["value"] = anim.track_get_key_value(track_idx, key_idx)

		Animation.TYPE_METHOD:
			var name_str: String = anim.method_track_get_name(track_idx, key_idx)
			var args_arr = anim.method_track_get_params(track_idx, key_idx)
			kf["method"] = name_str
			kf["args"] = SceneOps._serialize_value(args_arr)

		Animation.TYPE_BEZIER:
			kf["value"] = anim.bezier_track_get_key_value(track_idx, key_idx)
			var in_h := anim.bezier_track_get_key_in_handle(track_idx, key_idx)
			var out_h := anim.bezier_track_get_key_out_handle(track_idx, key_idx)
			kf["in_handle"] = {"x": in_h.x, "y": in_h.y}
			kf["out_handle"] = {"x": out_h.x, "y": out_h.y}

		_:
			kf["value"] = SceneOps._serialize_value(
				anim.track_get_key_value(track_idx, key_idx))

	return kf


static func _get_interpolation_name(interp: int) -> String:
	match interp:
		Animation.INTERPOLATION_NEAREST: return "nearest"
		Animation.INTERPOLATION_LINEAR: return "linear"
		Animation.INTERPOLATION_CUBIC: return "cubic"
		_: return "linear"


static func _get_update_mode_name(mode: int) -> String:
	match mode:
		Animation.UPDATE_CONTINUOUS: return "continuous"
		Animation.UPDATE_DISCRETE: return "discrete"
		Animation.UPDATE_CAPTURE: return "capture"
		_: return "continuous"
