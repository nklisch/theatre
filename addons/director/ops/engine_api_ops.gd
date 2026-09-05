class_name EngineApiOps

const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")

const DEFAULT_LIMIT := 25
const MAX_LIMIT := 100
const CATEGORIES := ["summary", "properties", "methods", "signals", "enums"]
const PROPERTY_HEADING_USAGE := \
	PROPERTY_USAGE_CATEGORY | PROPERTY_USAGE_GROUP | PROPERTY_USAGE_SUBGROUP


static func op_engine_api(params: Dictionary) -> Dictionary:
	var target_class: String = params.get("class_name", "")
	var category: String = params.get("category", "summary")
	var member = params.get("member", null)
	var raw_offset = params.get("offset", 0)
	var raw_limit = params.get("limit", DEFAULT_LIMIT)

	if not _is_integral_number(raw_offset):
		return OpsUtil._error(
			"offset must be an integer", "engine_api", {"offset": raw_offset})
	if not _is_integral_number(raw_limit):
		return OpsUtil._error(
			"limit must be an integer", "engine_api", {"limit": raw_limit})

	var offset := int(raw_offset)
	var limit := int(raw_limit)

	if target_class == "":
		return OpsUtil._error("class_name is required", "engine_api", params)
	if not ClassDB.class_exists(target_class):
		return OpsUtil._error(
			"Unknown ClassDB class: " + target_class,
			"engine_api", {"class_name": target_class})
	if category not in CATEGORIES:
		return OpsUtil._error(
			"category must be one of: " + ", ".join(CATEGORIES),
			"engine_api", {"category": category})
	if offset < 0:
		return OpsUtil._error("offset must be zero or greater", "engine_api", {"offset": offset})
	if limit < 1 or limit > MAX_LIMIT:
		return OpsUtil._error(
			"limit must be between 1 and %d" % MAX_LIMIT,
			"engine_api", {"limit": limit})
	if category == "summary" and member != null:
		return OpsUtil._error(
			"member requires a focused category", "engine_api", {"category": category})
	if category == "summary" and offset != 0:
		return OpsUtil._error(
			"summary does not support a non-zero offset", "engine_api", {"offset": offset})

	var counts := {
		"properties": _raw_members(target_class, "properties", false).size(),
		"methods": _raw_members(target_class, "methods", false).size(),
		"signals": _raw_members(target_class, "signals", false).size(),
		"enums": _raw_members(target_class, "enums", false).size(),
	}
	var response := {
		"engine_version": _engine_version(),
		"class": {
			"class_name": target_class,
			"parent_class": ClassDB.get_parent_class(target_class),
			"instantiable": ClassDB.can_instantiate(target_class),
		},
		"category": category,
		"counts": counts,
		"members": [],
	}

	if category == "summary":
		return {"success": true, "data": response}

	var raw_members := _raw_members(target_class, category, false)
	raw_members.sort_custom(func(a, b): return _member_name(a) < _member_name(b))

	if member != null:
		var exact_name := str(member)
		raw_members = raw_members.filter(func(item): return _member_name(item) == exact_name)
		if raw_members.is_empty():
			return OpsUtil._error(
				"Unknown %s member '%s' on %s" % [_category_label(category), exact_name, target_class],
				"engine_api", {"class_name": target_class, "category": category, "member": exact_name})

	var total := raw_members.size()
	if offset > total or (offset == total and total > 0):
		return OpsUtil._error(
			"offset %d is outside %s results (total %d)" % [offset, category, total],
			"engine_api", {"offset": offset, "total": total, "category": category})

	var declarations := _declarations(target_class, category)
	var page_end := mini(offset + limit, total)
	var members: Array = []
	for item in raw_members.slice(offset, page_end):
		members.append(_serialize_member(target_class, category, item, declarations))

	response["members"] = members
	response["page"] = {
		"offset": offset,
		"limit": limit,
		"total": total,
		"next_offset": page_end if page_end < total else null,
	}
	return {"success": true, "data": response}


static func _engine_version() -> Dictionary:
	var version := Engine.get_version_info()
	return {
		"major": int(version.get("major", 0)),
		"minor": int(version.get("minor", 0)),
		"patch": int(version.get("patch", 0)),
		"status": str(version.get("status", "")),
		"build": str(version.get("build", "")),
		"hash": str(version.get("hash", "")),
		"string": str(version.get("string", "")),
	}


static func _raw_members(target_class: String, category: String, no_inheritance: bool) -> Array:
	match category:
		"properties":
			var properties: Array = ClassDB.class_get_property_list(target_class, no_inheritance)
			return properties.filter(func(property):
				return (int(property.get("usage", 0)) & PROPERTY_HEADING_USAGE) == 0)
		"methods": return ClassDB.class_get_method_list(target_class, no_inheritance)
		"signals": return ClassDB.class_get_signal_list(target_class, no_inheritance)
		"enums": return Array(ClassDB.class_get_enum_list(target_class, no_inheritance))
		_: return []


static func _is_integral_number(value) -> bool:
	if typeof(value) == TYPE_INT:
		return true
	if typeof(value) == TYPE_FLOAT:
		return not is_nan(value) and not is_inf(value) and value == floor(value)
	return false


static func _member_name(member) -> String:
	return str(member.get("name", "")) if member is Dictionary else str(member)


static func _category_label(category: String) -> String:
	match category:
		"properties": return "property"
		"methods": return "method"
		"signals": return "signal"
		"enums": return "enum"
		_: return category


static func _declarations(target_class: String, category: String) -> Dictionary:
	var declarations := {}
	var current := target_class
	while current != "":
		for item in _raw_members(current, category, true):
			var member_name := _member_name(item)
			if member_name not in declarations:
				declarations[member_name] = current
		current = ClassDB.get_parent_class(current)
	return declarations


static func _serialize_member(
		target_class: String, category: String, member, declarations: Dictionary) -> Dictionary:
	var name := _member_name(member)
	var declared_by: String = declarations.get(name, target_class)
	match category:
		"properties":
			return {
				"kind": "property",
				"name": name,
				"declared_by": declared_by,
				"value_type": int(member.get("type", TYPE_NIL)),
				"type_name": type_string(int(member.get("type", TYPE_NIL))),
				"class_name": str(member.get("class_name", "")),
				"hint": int(member.get("hint", PROPERTY_HINT_NONE)),
				"hint_string": str(member.get("hint_string", "")),
				"usage": int(member.get("usage", 0)),
				"default_value": _serialize_default(
					ClassDB.class_get_property_default_value(target_class, name)),
			}
		"methods":
			var arguments: Array = []
			for argument in member.get("args", []):
				arguments.append(_serialize_argument(argument))
			var defaults: Array = []
			for default_value in member.get("default_args", []):
				defaults.append(_serialize_default(default_value))
			return {
				"kind": "method",
				"name": name,
				"declared_by": declared_by,
				"flags": int(member.get("flags", 0)),
				"arguments": arguments,
				"return_value": _serialize_argument(member.get("return", {})),
				"default_arguments": defaults,
			}
		"signals":
			var arguments: Array = []
			for argument in member.get("args", []):
				arguments.append(_serialize_argument(argument))
			return {
				"kind": "signal",
				"name": name,
				"declared_by": declared_by,
				"arguments": arguments,
			}
		"enums":
			var values: Array = []
			for constant_name in ClassDB.class_get_enum_constants(target_class, name, false):
				values.append({
					"name": str(constant_name),
					"value": ClassDB.class_get_integer_constant(target_class, constant_name),
				})
			return {
				"kind": "enum",
				"name": name,
				"declared_by": declared_by,
				"bitfield": ClassDB.is_class_enum_bitfield(target_class, name, false),
				"values": values,
			}
	return {}


static func _serialize_argument(argument: Dictionary) -> Dictionary:
	var value_type := int(argument.get("type", TYPE_NIL))
	return {
		"name": str(argument.get("name", "")),
		"value_type": value_type,
		"type_name": type_string(value_type),
		"class_name": str(argument.get("class_name", "")),
		"hint": int(argument.get("hint", PROPERTY_HINT_NONE)),
		"hint_string": str(argument.get("hint_string", "")),
		"usage": int(argument.get("usage", 0)),
	}


static func _serialize_default(value) -> Dictionary:
	var native_type := type_string(typeof(value))
	# ClassDB also returns nil when it has no reportable default, so do not claim
	# that nil is a round-trippable declared default.
	if typeof(value) == TYPE_NIL:
		return {
			"native_type": native_type,
			"representation": "unavailable",
			"value": null,
			"text": null,
		}
	if typeof(value) in [TYPE_BOOL, TYPE_INT, TYPE_STRING]:
		return {
			"native_type": native_type,
			"representation": "json",
			"value": value,
			"text": null,
		}
	if typeof(value) == TYPE_FLOAT:
		if is_nan(value) or is_inf(value):
			return _text_default(native_type, str(value))
		return {
			"native_type": native_type,
			"representation": "json",
			"value": value,
			"text": null,
		}
	if typeof(value) in [
		TYPE_VECTOR2, TYPE_VECTOR3, TYPE_COLOR, TYPE_NODE_PATH, TYPE_RECT2,
		TYPE_TRANSFORM2D, TYPE_BASIS, TYPE_TRANSFORM3D, TYPE_ARRAY, TYPE_DICTIONARY,
	]:
		return {
			"native_type": native_type,
			"representation": "serialized",
			"value": SceneOps._serialize_value(value),
			"text": null,
		}
	if value is Resource:
		return {
			"native_type": native_type,
			"representation": "serialized",
			"value": SceneOps._serialize_value(value),
			"text": null,
		}
	return _text_default(native_type, str(value))


static func _text_default(native_type: String, text: String) -> Dictionary:
	return {
		"native_type": native_type,
		"representation": "text" if text != "" else "unavailable",
		"value": null,
		"text": text if text != "" else null,
	}
