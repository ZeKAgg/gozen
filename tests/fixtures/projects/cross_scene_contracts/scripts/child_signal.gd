extends Node

func _ready() -> void:
	get_parent().connect("custom_signal", Callable(self, "_on_custom"))

func _on_custom() -> void:
	pass
