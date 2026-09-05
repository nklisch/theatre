extends CharacterBody3D

## Player script used in test scenes.
@export var health: int = 100
@export var speed: float = 5.0
var _paused_after_idle_stage_stop := true


func _physics_process(delta: float) -> void:
	if Input.is_action_pressed("test_jump"):
		position.x += speed * delta


func is_sequence_test_input_pressed() -> bool:
	return Input.is_action_pressed("test_jump")


func set_sequence_test_physics_ticks_per_second(ticks: int) -> int:
	var previous := Engine.physics_ticks_per_second
	Engine.physics_ticks_per_second = ticks
	return previous


func restart_stage_listener_deferred() -> bool:
	call_deferred("_restart_stage_listener")
	return true


func _restart_stage_listener() -> void:
	var runtime := get_node("/root/StageRuntime")
	var port: int = runtime.tcp_server.get_port()
	runtime.tcp_server.stop()
	_paused_after_idle_stage_stop = get_tree().paused
	runtime.tcp_server.start(port)


func was_paused_after_idle_stage_stop() -> bool:
	return _paused_after_idle_stage_stop
