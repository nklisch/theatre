extends CharacterBody3D

@export var health: int = 80
@export var speed: float = 2.0
@export var patrol_range: float = 4.0

var _start_pos: Vector3
var _direction: float = 1.0
var _elapsed: float = 0.0

func _ready() -> void:
    _start_pos = global_position

func _physics_process(delta: float) -> void:
    _elapsed += delta
    var offset = sin(_elapsed * speed) * patrol_range
    global_position = _start_pos + Vector3(offset, 0, 0)
