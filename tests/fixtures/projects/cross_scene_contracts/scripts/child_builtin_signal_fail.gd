extends Node

func _ready() -> void:
	get_parent().connect("focus_entered", Callable(self, "_on_focus"))

func _on_focus() -> void:
	pass
