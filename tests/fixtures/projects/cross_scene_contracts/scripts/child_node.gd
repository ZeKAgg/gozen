extends Node

func _ready() -> void:
	get_parent().get_node("MustExist")
