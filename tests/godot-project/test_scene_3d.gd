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
