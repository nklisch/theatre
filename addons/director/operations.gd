extends SceneTree

## Director headless operations dispatcher.
## Called via: godot --headless --path <project> --script addons/director/operations.gd -- <op> '<json>'

const Dispatcher = preload("res://addons/director/ops/dispatcher.gd")


func _init():
	var args = _parse_args()
	if args.error != "":
		_print_error(args.error, "parse_args", {})
		quit(1)
		return

	var result = Dispatcher.dispatch(args.operation, args.params)

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
