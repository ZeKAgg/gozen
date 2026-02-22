@tool
extends AcceptDialog
## Settings dialog for configuring Gozen plugin options.
## Reads from and writes to Godot editor settings so values persist.

## Set by gozen_panel.gd before showing the dialog.
var plugin: EditorPlugin

var _path_edit: LineEdit
var _btn_browse: Button
var _check_on_save: CheckBox
var _auto_format: CheckBox


func _ready() -> void:
	# Resolve nodes
	_path_edit = $VBox/PathRow/PathEdit
	_btn_browse = $VBox/PathRow/BtnBrowse
	_check_on_save = $VBox/CheckOnSave
	_auto_format = $VBox/AutoFormatOnSave

	# Load current values from the plugin's settings helpers
	if plugin:
		_path_edit.text = plugin.get_binary_path()
		_check_on_save.button_pressed = plugin.get_check_on_save()
		_auto_format.button_pressed = plugin.get_auto_format_on_save()

	# Connect signals
	_btn_browse.pressed.connect(_on_browse)
	confirmed.connect(_on_confirmed)
	canceled.connect(_on_canceled)


func _on_browse() -> void:
	var fd := FileDialog.new()
	fd.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	fd.access = FileDialog.ACCESS_FILESYSTEM
	if OS.get_name() == "Windows":
		fd.add_filter("*.exe", "Executables")
	fd.add_filter("*", "All Files")
	fd.file_selected.connect(_on_file_selected)
	add_child(fd)
	fd.popup_centered(Vector2i(600, 400))


func _on_file_selected(path: String) -> void:
	_path_edit.text = path


func _on_confirmed() -> void:
	if plugin:
		plugin.set_binary_path(_path_edit.text)
		plugin.set_check_on_save(_check_on_save.button_pressed)
		plugin.set_auto_format_on_save(_auto_format.button_pressed)
	queue_free()


func _on_canceled() -> void:
	queue_free()
