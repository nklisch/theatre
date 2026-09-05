extends Node3D

## Root scene script for 3D wire tests and GDScript tests.
## Provides a ping() method and a known node hierarchy with exported vars.


func _ready() -> void:
	pass


## Returns "pong". Used by test_actions.rs call_method test.
func ping() -> String:
	return "pong"


## Adds two numbers. Used by test_actions.rs call_method_with_args test.
func add(a: int, b: int) -> int:
	return a + b


## Focused real-engine hooks for Stage's current-run diagnostic journey.
func emit_runtime_diagnostic_basics() -> void:
	push_error("stage-runtime-diagnostics deliberate error")
	push_warning("stage-runtime-diagnostics deliberate warning")


func emit_runtime_script_error() -> void:
	var empty_values: Array = []
	var _missing_value: Variant = empty_values[1]


func emit_worker_runtime_diagnostic() -> int:
	return WorkerThreadPool.add_task(_worker_runtime_diagnostic)


func _worker_runtime_diagnostic() -> void:
	push_error("stage-runtime-diagnostics worker error")


func emit_runtime_diagnostic_overflow(count: int) -> void:
	for index in count:
		push_warning("stage-runtime-diagnostics overflow %d" % index)
