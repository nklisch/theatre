extends CharacterBody2D

@export var speed: float = 200.0
@export var health: int = 100

func _physics_process(_delta: float) -> void:
	var input_dir := Vector2(
		Input.get_axis("ui_left", "ui_right"),
		Input.get_axis("ui_up", "ui_down")
	)
	velocity = input_dir * speed
	move_and_slide()
