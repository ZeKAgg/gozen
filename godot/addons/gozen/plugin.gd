@tool
extends EditorPlugin
## Main Gozen editor plugin.
## Runs the gozen CLI on save events and exposes Check / Format actions
## through a bottom dock panel.

const GozenPanelScene := preload("res://addons/gozen/ui/gozen_panel.tscn")

var _panel: Control
var _gozen_thread: Thread


# ---------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------

func _enter_tree() -> void:
	_panel = GozenPanelScene.instantiate()
	_panel.plugin = self
	add_control_to_bottom_panel(_panel, "Gozen")

	resource_saved.connect(_on_resource_saved)
	scene_saved.connect(_on_scene_saved)

	# Ensure default editor settings exist
	_ensure_settings()

	# Try to auto-detect the gozen binary if the default doesn't work
	_auto_detect_binary()


func _exit_tree() -> void:
	if resource_saved.is_connected(_on_resource_saved):
		resource_saved.disconnect(_on_resource_saved)
	if scene_saved.is_connected(_on_scene_saved):
		scene_saved.disconnect(_on_scene_saved)

	if _panel:
		remove_control_from_bottom_panel(_panel)
		_panel.queue_free()
		_panel = null

	_join_thread()


# ---------------------------------------------------------------------------
# Save hooks
# ---------------------------------------------------------------------------

func _on_resource_saved(resource: Resource) -> void:
	var path: String = resource.resource_path
	if not _is_gozen_file(path):
		return
	# Format first so that check reports diagnostics on the formatted file
	if get_auto_format_on_save():
		run_gozen_format([path])
	if get_check_on_save():
		run_gozen_check([path])


func _on_scene_saved(filepath: String) -> void:
	# Scene saves don't need linting by default; the attached scripts
	# are saved separately and caught by resource_saved.
	pass


# ---------------------------------------------------------------------------
# Running gozen
# ---------------------------------------------------------------------------

## Run `gozen check --reporter json` on the given paths and display results.
func run_gozen_check(paths: PackedStringArray) -> void:
	if _panel:
		_panel.set_running(true)
	var args := PackedStringArray(["check", "--reporter", "json"])
	args.append_array(paths)
	var result := _execute_gozen(args)
	_handle_json_result(result)
	if _panel:
		_panel.set_running(false)


## Run `gozen lint --reporter json` on the given paths and display results.
func run_gozen_lint(paths: PackedStringArray) -> void:
	if _panel:
		_panel.set_running(true)
	var args := PackedStringArray(["lint", "--reporter", "json"])
	args.append_array(paths)
	var result := _execute_gozen(args)
	_handle_json_result(result)
	if _panel:
		_panel.set_running(false)


## Run `gozen format` on the given paths (no JSON output needed).
func run_gozen_format(paths: PackedStringArray) -> void:
	if _panel:
		_panel.set_running(true)
	var args := PackedStringArray(["format"])
	args.append_array(paths)
	_execute_gozen(args)
	if _panel:
		_panel.set_status("Formatted %d file(s)." % paths.size())
		_panel.set_running(false)
	# Reload the formatted files in the editor
	EditorInterface.get_resource_filesystem().scan()


## Run `gozen check --reporter json .` on the whole project in a background
## thread so the editor stays responsive.
func run_gozen_check_project() -> void:
	_join_thread()
	if _panel:
		_panel.set_running(true)
		_panel.clear_results()
	# Capture binary path on main thread (EditorInterface is not thread-safe)
	var binary := get_binary_path()
	_gozen_thread = Thread.new()
	_gozen_thread.start(_thread_check_project.bind(binary))


## Run `gozen format .` on the whole project in a background thread.
func run_gozen_format_project() -> void:
	_join_thread()
	if _panel:
		_panel.set_running(true)
	# Capture binary path on main thread (EditorInterface is not thread-safe)
	var binary := get_binary_path()
	_gozen_thread = Thread.new()
	_gozen_thread.start(_thread_format_project.bind(binary))


## Jump the script editor to a file and line from a diagnostic.
func goto_issue(file_path: String, line: int) -> void:
	if file_path.ends_with(".gdshader"):
		# Shader files are Shader resources, not Script — open via edit_resource
		var shader := load(file_path) as Shader
		if shader:
			EditorInterface.set_main_screen_editor("Script")
			EditorInterface.edit_resource(shader)
			# Defer line navigation so the editor finishes opening the shader
			_goto_line_deferred.call_deferred(line)
		else:
			push_warning("Gozen: could not load shader at %s" % file_path)
		return
	EditorInterface.set_main_screen_editor("Script")
	var script := load(file_path) as Script
	if script:
		# edit_script line is 0-based; our JSON is 1-based
		EditorInterface.edit_script(script, line - 1, 0, true)
	else:
		push_warning("Gozen: could not load script at %s" % file_path)


## Return the path of the currently active script, or "" if none.
func get_active_script_path() -> String:
	var script_editor := EditorInterface.get_script_editor()
	if script_editor == null:
		return ""
	var current := script_editor.get_current_script()
	if current == null:
		return ""
	return current.resource_path


## Navigate the active editor to a specific line (deferred helper for shaders).
func _goto_line_deferred(line: int) -> void:
	var script_editor := EditorInterface.get_script_editor()
	if script_editor:
		# goto_line is 0-based; our JSON diagnostics are 1-based
		script_editor.goto_line(line - 1)


# ---------------------------------------------------------------------------
# Binary detection and validation
# ---------------------------------------------------------------------------

## Try to locate the gozen binary automatically if the default doesn't work.
func _auto_detect_binary() -> void:
	var current: String = get_binary_path()

	# If using a custom (non-default) path, just validate it
	if current != "gozen":
		if _test_binary(current):
			return
		if _panel:
			_panel.set_status("Gozen binary not found at: %s. Check Settings." % current)
		return

	# Default "gozen" — check if it works on PATH first
	if _test_binary(current):
		return

	# Try common locations
	var candidates := PackedStringArray()
	var ext: String = ".exe" if OS.get_name() == "Windows" else ""

	# Try ~/.cargo/bin/gozen
	var home: String = ""
	if OS.get_name() == "Windows":
		home = OS.get_environment("USERPROFILE")
	else:
		home = OS.get_environment("HOME")
	if not home.is_empty():
		candidates.append(home.path_join(".cargo").path_join("bin").path_join("gozen" + ext))

	# Walk up from the Godot project looking for target/release or target/debug
	var project_dir: String = ProjectSettings.globalize_path("res://")
	var dir: String = project_dir
	for i in range(5):
		candidates.append(dir.path_join("target").path_join("release").path_join("gozen" + ext))
		candidates.append(dir.path_join("target").path_join("debug").path_join("gozen" + ext))
		var parent: String = dir.get_base_dir()
		if parent == dir:
			break
		dir = parent

	for idx in range(candidates.size()):
		var candidate: String = candidates[idx]
		if FileAccess.file_exists(candidate) and _test_binary(candidate):
			set_binary_path(candidate)
			if _panel:
				_panel.set_status("Auto-detected gozen at: " + candidate)
			return

	# Nothing found
	if _panel:
		_panel.show_not_found()


## Quick check whether a binary path can execute gozen --version.
func _test_binary(binary: String) -> bool:
	var output: Array = []
	var exit_code: int = OS.execute(binary, PackedStringArray(["--version"]), output, false)
	return exit_code != -1


## Validate the binary path and return whether it works.
func validate_binary(binary: String) -> bool:
	if binary.is_empty():
		return false
	return _test_binary(binary)


# ---------------------------------------------------------------------------
# Settings helpers
# ---------------------------------------------------------------------------

func _ensure_settings() -> void:
	var es := EditorInterface.get_editor_settings()
	if not es.has_setting("gozen/general/binary_path"):
		es.set_setting("gozen/general/binary_path", "gozen")
	if not es.has_setting("gozen/general/check_on_save"):
		es.set_setting("gozen/general/check_on_save", true)
	if not es.has_setting("gozen/general/auto_format_on_save"):
		es.set_setting("gozen/general/auto_format_on_save", false)


func get_binary_path() -> String:
	return EditorInterface.get_editor_settings().get_setting("gozen/general/binary_path")


func get_check_on_save() -> bool:
	return EditorInterface.get_editor_settings().get_setting("gozen/general/check_on_save")


func get_auto_format_on_save() -> bool:
	return EditorInterface.get_editor_settings().get_setting("gozen/general/auto_format_on_save")


func set_binary_path(value: String) -> void:
	EditorInterface.get_editor_settings().set_setting("gozen/general/binary_path", value)


func set_check_on_save(value: bool) -> void:
	EditorInterface.get_editor_settings().set_setting("gozen/general/check_on_save", value)


func set_auto_format_on_save(value: bool) -> void:
	EditorInterface.get_editor_settings().set_setting("gozen/general/auto_format_on_save", value)


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

func _is_gozen_file(path: String) -> bool:
	return path.ends_with(".gd") or path.ends_with(".gdshader")


func _execute_gozen(args: PackedStringArray) -> Dictionary:
	var binary := get_binary_path()
	return _execute_gozen_with_binary(binary, args)


## Thread-safe version that takes the binary path as a parameter.
## read_stderr is false so only stdout is captured (keeps JSON output clean).
func _execute_gozen_with_binary(binary: String, args: PackedStringArray) -> Dictionary:
	var output: Array = []
	var exit_code := OS.execute(binary, args, output, false)
	return {
		"exit_code": exit_code,
		"output": output[0] if output.size() > 0 else "",
	}


func _handle_json_result(result: Dictionary) -> void:
	if _panel == null:
		return

	var exit_code: int = result.get("exit_code", -1)
	var raw: String = result.get("output", "")

	# Binary not found or could not start
	if exit_code == -1:
		_panel.show_not_found()
		return

	# Gozen crashed or hit an internal error
	if exit_code >= 2 and raw.is_empty():
		_panel.set_status("Gozen crashed (exit code %d). Check the Output panel." % exit_code)
		return

	# Exit 0 with no output — nothing to show
	if raw.is_empty():
		_panel.display_results({"diagnostics": [], "summary": {}})
		return

	# gozen exits 0 = success, 1 = diagnostics found
	var data = JSON.parse_string(raw)
	if data == null or not data is Dictionary:
		_panel.set_status("Could not parse gozen output: %s" % raw.left(200))
		push_warning("Gozen: unexpected output: %s" % raw.left(500))
		return

	_panel.display_results(data)


func _thread_check_project(binary: String) -> void:
	var args := PackedStringArray(["check", "--reporter", "json", "."])
	var result := _execute_gozen_with_binary(binary, args)
	call_deferred("_on_check_thread_finished", result)


func _thread_format_project(binary: String) -> void:
	var args := PackedStringArray(["format", "."])
	_execute_gozen_with_binary(binary, args)
	call_deferred("_on_format_thread_finished")


func _on_check_thread_finished(result: Dictionary) -> void:
	_join_thread()
	_handle_json_result(result)
	if _panel:
		_panel.set_running(false)


func _on_format_thread_finished() -> void:
	_join_thread()
	if _panel:
		_panel.set_status("Project formatted.")
		_panel.set_running(false)
	EditorInterface.get_resource_filesystem().scan()


func _join_thread() -> void:
	if _gozen_thread != null and _gozen_thread.is_started():
		_gozen_thread.wait_to_finish()
	_gozen_thread = null
