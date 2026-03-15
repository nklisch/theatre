extends CharacterBody3D

@export var health: int = 60
@export var speed: float = 0.0

func take_damage(amount: int) -> void:
    health -= amount
    if health <= 0:
        health = 0
