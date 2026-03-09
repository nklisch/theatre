class_name OpsUtil


static func _error(message: String, operation: String,
		context: Dictionary) -> Dictionary:
	return {"success": false, "error": message, "operation": operation,
		"context": context}


static func _validate_node_type(node: Node, expected: String,
		operation: String, context: Dictionary) -> Dictionary:
	## Generic node type validator using is_class() for the check.
	## Returns { success: true } or error dict.
	if node.is_class(expected):
		return {"success": true}
	return _error("Node is %s, expected %s" % [node.get_class(), expected],
		operation, context)
