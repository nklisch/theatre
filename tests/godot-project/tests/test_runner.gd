## GDScript test runner for the Stage addon.
##
## Discovers all test_*.gd files in res://tests/, instantiates each one,
## and calls every method starting with "test_". Each method returns "" on
## success or an error message on failure.
##
## Outputs TAP (Test Anything Protocol) to stdout. Exits 0 on all-pass, 1 if
## any test fails.
##
## Usage:
##   godot --headless --script res://tests/test_runner.gd \
##         --path tests/godot-project --quit-after 30
extends SceneTree

var _test_count := 0
var _pass_count := 0
var _fail_count := 0
var _results: Array[String] = []


func _init() -> void:
	# Wait two frames so all _ready() calls have run
	await process_frame
	await process_frame

	await _discover_and_run()
	_print_results()

	quit(0 if _fail_count == 0 else 1)


func _discover_and_run() -> void:
	var dir := DirAccess.open("res://tests/")
	if not dir:
		push_error("[TestRunner] Cannot open res://tests/")
		quit(1)
		return

	var scripts: Array[String] = []
	dir.list_dir_begin()
	var file := dir.get_next()
	while file != "":
		if file.begins_with("test_") and file.ends_with(".gd") \
				and file != "test_runner.gd":
			scripts.append("res://tests/" + file)
		file = dir.get_next()
	scripts.sort()  # deterministic order

	for path in scripts:
		await _run_test_script(path)


func _run_test_script(path: String) -> void:
	var script: GDScript = load(path)
	if script == null:
		_record(false, path, "test_load", "failed to load script")
		return

	var instance = script.new()

	# If the test script has a setup method, pass the root window
	if instance.has_method("setup"):
		instance.setup(get_root())

	var method_list: Array[Dictionary] = instance.get_method_list()
	method_list.sort_custom(func(a, b): return a["name"] < b["name"])

	for method in method_list:
		var name: String = method["name"]
		if not name.begins_with("test_"):
			continue

		_test_count += 1
		var err: String = ""

		# Use Callable + await so both sync and async (coroutine) test functions
		# work correctly. For synchronous functions, await returns the value
		# immediately. For coroutine functions, it waits for completion and
		# captures the actual return value (the String error message or "").
		var result = await Callable(instance, name).call()

		if result == null:
			err = ""
		elif result is String:
			err = result
		else:
			err = str(result)

		_record(err == "", path, name, err)

	# Cleanup
	if instance.has_method("teardown"):
		instance.teardown()
	if instance is Node and instance.is_inside_tree():
		instance.queue_free()


func _record(passed: bool, path: String, test_name: String, err: String) -> void:
	var label := "%s/%s" % [_basename(path), test_name]
	if passed:
		_pass_count += 1
		_results.append("ok %d - %s" % [_test_count, label])
	else:
		_fail_count += 1
		_results.append("not ok %d - %s  # %s" % [_test_count, label, err])


func _print_results() -> void:
	print("TAP version 13")
	print("1..%d" % _test_count)
	for line in _results:
		print(line)
	print("# %d passed, %d failed" % [_pass_count, _fail_count])


static func _basename(path: String) -> String:
	return path.get_file().get_basename()
