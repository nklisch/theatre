extends RefCounted
## Filter live metadata in-engine: only exported values cross into Rust.
## Do not substitute the script's static list or cache this result: native and
## dynamic properties, including _validate_property changes, belong to the node.

static func exported_state(node: Node) -> Dictionary:
	var state := {}
	for property: Dictionary in node.get_property_list():
		var usage: int = property.get("usage", 0)
		if usage & (PROPERTY_USAGE_SCRIPT_VARIABLE | PROPERTY_USAGE_EDITOR) != (PROPERTY_USAGE_SCRIPT_VARIABLE | PROPERTY_USAGE_EDITOR):
			continue
		var property_name := StringName(property.get("name", ""))
		if not property_name.is_empty():
			state[property_name] = node.get(property_name)
	return state
