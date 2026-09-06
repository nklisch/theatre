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


## Built on demand so ordinary scene journeys retain their original hierarchy.
func create_area_monitoring_fixture() -> void:
	var fixture := Node3D.new()
	fixture.name = "MonitoringFixture"
	add_child(fixture)
	fixture.position = Vector3(1000, 1000, 1000)
	var body := StaticBody3D.new()
	body.name = "Body3D"
	var body_shape := CollisionShape3D.new()
	body_shape.shape = SphereShape3D.new()
	body.add_child(body_shape)
	fixture.add_child(body)
	var disabled := Area3D.new()
	disabled.name = "Disabled3D"
	disabled.monitoring = false
	fixture.add_child(disabled)
	var enabled := Area3D.new()
	enabled.name = "Enabled3D"
	var shape := CollisionShape3D.new()
	shape.shape = SphereShape3D.new()
	enabled.add_child(shape)
	# An enabled descendant must still be found below a disabled area.
	disabled.add_child(enabled)
	var empty := Area3D.new()
	empty.name = "Empty3D"
	fixture.add_child(empty)
	var disabled_2d := Area2D.new()
	disabled_2d.name = "Disabled2D"
	disabled_2d.monitoring = false
	fixture.add_child(disabled_2d)
	var enabled_2d := Area2D.new()
	enabled_2d.name = "Enabled2D"
	var shape_2d := CollisionShape2D.new()
	shape_2d.shape = CircleShape2D.new()
	enabled_2d.add_child(shape_2d)
	fixture.add_child(enabled_2d)
	var body_2d := StaticBody2D.new()
	body_2d.name = "Body2D"
	var body_shape_2d := CollisionShape2D.new()
	body_shape_2d.shape = CircleShape2D.new()
	body_2d.add_child(body_shape_2d)
	fixture.add_child(body_2d)
	var empty_2d := Area2D.new()
	empty_2d.name = "Empty2D"
	fixture.add_child(empty_2d)
