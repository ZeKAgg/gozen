extends Node

func _ready() -> void:
	get_parent().get_node("%UniqueName")
	get_parent().get_node("/root/World")
