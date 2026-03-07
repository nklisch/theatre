extends CharacterBody2D

@export var speed: float = 50.0
@export var health: int = 80
@export var patrol_range: float = 100.0

var _start_x: float
var _direction: float = 1.0

func _ready() -> void:
	_start_x = global_position.x

func _physics_process(_delta: float) -> void:
	if global_position.x > _start_x + patrol_range:
		_direction = -1.0
	elif global_position.x < _start_x - patrol_range:
		_direction = 1.0
	velocity.x = speed * _direction
	move_and_slide()
