extends Node2D
## Fixture: target node for signal connection tests.
## Godot 4.7 validates bound callables at connect time, so the method must exist.

var received: Array = []

func on_press(extra, count) -> void:
	received.append([extra, count])
