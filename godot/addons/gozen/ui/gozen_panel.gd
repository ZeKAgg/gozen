@tool
extends VBoxContainer
## Bottom dock panel that displays gozen lint/check results and provides
## toolbar buttons for running gozen commands.

const SettingsDialogScene := preload("res://addons/gozen/ui/settings_dialog.tscn")

## Set by plugin.gd after instantiation.
var plugin: EditorPlugin

# Node references (resolved in _ready)
var _btn_check_project: Button
var _btn_format_project: Button
var _btn_lint_file: Button
var _btn_format_file: Button
var _btn_settings: Button
var _summary_label: Label
var _error_tree: Tree
var _placeholder_label: Label
var _status_bar: Label

# Editor theme icons (cached after _ready)
var _icon_error: Texture2D
var _icon_warning: Texture2D
var _icon_script: Texture2D
var _icon_shader: Texture2D


# ---------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------

func _ready() -> void:
	# Resolve nodes manually to avoid unique-name issues across Godot versions.
	_btn_check_project = $Toolbar/BtnCheckProject
	_btn_format_project = $Toolbar/BtnFormatProject
	_btn_lint_file = $Toolbar/BtnLintFile
	_btn_format_file = $Toolbar/BtnFormatFile
	_btn_settings = $Toolbar/BtnSettings
	_summary_label = $Toolbar/SummaryLabel
	_error_tree = $ErrorTree
	_placeholder_label = $PlaceholderLabel
	_status_bar = $StatusBar

	# Cache editor theme icons
	var theme: Theme = EditorInterface.get_editor_theme()
	_icon_error = theme.get_icon("StatusError", "EditorIcons")
	_icon_warning = theme.get_icon("StatusWarning", "EditorIcons")
	_icon_script = theme.get_icon("Script", "EditorIcons")
	_icon_shader = theme.get_icon("Shader", "EditorIcons")

	# Connect toolbar signals
	_btn_check_project.pressed.connect(_on_check_project)
	_btn_format_project.pressed.connect(_on_format_project)
	_btn_lint_file.pressed.connect(_on_lint_file)
	_btn_format_file.pressed.connect(_on_format_file)
	_btn_settings.pressed.connect(_on_settings)

	# Tree setup
	_setup_tree()

	# Double-click / enter to jump to file
	_error_tree.item_activated.connect(_on_tree_item_activated)

	# Start in idle state
	_show_placeholder("Run a check to get started.")


# ---------------------------------------------------------------------------
# Tree setup
# ---------------------------------------------------------------------------

func _setup_tree() -> void:
	# 4 columns: Icon | Location | Rule | Message
	_error_tree.set_column_custom_minimum_width(0, 32)
	_error_tree.set_column_custom_minimum_width(1, 180)
	_error_tree.set_column_custom_minimum_width(2, 160)
	_error_tree.set_column_custom_minimum_width(3, 300)

	_error_tree.set_column_expand(0, false)
	_error_tree.set_column_expand(1, true)
	_error_tree.set_column_expand(2, true)
	_error_tree.set_column_expand(3, true)


# ---------------------------------------------------------------------------
# Public API (called by plugin.gd)
# ---------------------------------------------------------------------------

## Remove all items from the error tree.
func clear_results() -> void:
	_error_tree.clear()
	_summary_label.text = ""


## Enable or disable toolbar buttons during execution.
func set_running(running: bool) -> void:
	_btn_check_project.disabled = running
	_btn_format_project.disabled = running
	_btn_lint_file.disabled = running
	_btn_format_file.disabled = running
	if running:
		set_status("Running gozen...")


## Show results parsed from gozen's JSON output.
func display_results(data: Dictionary) -> void:
	clear_results()

	var diagnostics: Array = data.get("diagnostics", [])
	var summary: Dictionary = data.get("summary", {})

	# Summary values
	var errors: int = summary.get("errors", 0)
	var warnings: int = summary.get("warnings", 0)
	var duration: int = summary.get("durationMs", 0)
	var files_checked: int = summary.get("filesChecked", 0)

	if diagnostics.is_empty():
		# Success state — no issues found
		_summary_label.text = ""
		_show_placeholder("All clear -- no issues in %d file(s) (%dms)." % [files_checked, duration])
		set_status("All clear -- 0 issues in %d file(s) (%dms)" % [files_checked, duration])
		return

	# Sort diagnostics by file, then line
	diagnostics.sort_custom(_cmp_diagnostics)

	# Group diagnostics by file path
	var grouped: Dictionary = {}
	var diag_idx: int = 0
	while diag_idx < diagnostics.size():
		var diag: Dictionary = diagnostics[diag_idx]
		var file_path: String = diag.get("file", "<unknown>")
		if not grouped.has(file_path):
			grouped[file_path] = []
		grouped[file_path].append(diag)
		diag_idx += 1

	# Build the tree
	var root := _error_tree.create_item()
	_error_tree.hide_root = true

	var file_paths: Array = grouped.keys()
	file_paths.sort()

	var fp_idx: int = 0
	while fp_idx < file_paths.size():
		var file_path: String = file_paths[fp_idx]
		var file_diags: Array = grouped[file_path]
		var file_item := _error_tree.create_item(root)

		# File icon
		var file_icon: Texture2D = _icon_shader if file_path.ends_with(".gdshader") else _icon_script
		file_item.set_icon(0, file_icon)

		# File path and issue count
		file_item.set_text(1, file_path)
		file_item.set_text(2, "(%d issues)" % file_diags.size())

		# Store metadata for navigation (line -1 = file group, not a diagnostic)
		file_item.set_metadata(0, file_path)
		file_item.set_metadata(1, -1)

		var d_idx: int = 0
		while d_idx < file_diags.size():
			var diag: Dictionary = file_diags[d_idx]
			var item := _error_tree.create_item(file_item)
			var severity: String = diag.get("severity", "warning")
			var span: Dictionary = diag.get("span", {})
			var line: int = span.get("start_line", 0)
			var rule: String = diag.get("rule", "")
			var message: String = diag.get("message", "")

			# Column 0: severity icon
			if severity == "error":
				item.set_icon(0, _icon_error)
			else:
				item.set_icon(0, _icon_warning)

			# Column 1: line number
			item.set_text(1, "Line %d" % line)

			# Column 2: rule
			item.set_text(2, rule)

			# Column 3: message
			item.set_text(3, message)

			# Store metadata for navigation
			item.set_metadata(0, file_path)
			item.set_metadata(1, line)
			d_idx += 1

		fp_idx += 1

	# Show the tree, hide placeholder
	_show_tree()

	# Summary label
	_summary_label.text = "%d error(s), %d warning(s) — %dms" % [errors, warnings, duration]

	# Status bar
	set_status("Last run: %d file(s) checked in %dms" % [files_checked, duration])


## Show a "gozen not found" message in the status bar.
func show_not_found() -> void:
	clear_results()
	_show_placeholder("Gozen binary not found. Open Settings to configure the path.")
	set_status("Gozen binary not found. Install with: cargo install gozen")


## Update the status bar text.
func set_status(text: String) -> void:
	if _status_bar:
		_status_bar.text = text


# ---------------------------------------------------------------------------
# Placeholder / tree visibility
# ---------------------------------------------------------------------------

func _show_placeholder(text: String) -> void:
	if _placeholder_label:
		_placeholder_label.text = text
		_placeholder_label.visible = true
	if _error_tree:
		_error_tree.visible = false


func _show_tree() -> void:
	if _placeholder_label:
		_placeholder_label.visible = false
	if _error_tree:
		_error_tree.visible = true


# ---------------------------------------------------------------------------
# Button handlers
# ---------------------------------------------------------------------------

func _on_check_project() -> void:
	if plugin:
		plugin.run_gozen_check_project()


func _on_format_project() -> void:
	if plugin:
		plugin.run_gozen_format_project()


func _on_lint_file() -> void:
	if plugin == null:
		return
	var path: String = plugin.get_active_script_path()
	if path.is_empty():
		set_status("No active script to lint.")
		return
	plugin.run_gozen_lint([path])


func _on_format_file() -> void:
	if plugin == null:
		return
	var path: String = plugin.get_active_script_path()
	if path.is_empty():
		set_status("No active script to format.")
		return
	plugin.run_gozen_format([path])


func _on_settings() -> void:
	if plugin == null:
		return
	var dialog := SettingsDialogScene.instantiate()
	dialog.plugin = plugin
	add_child(dialog)
	dialog.popup_centered(Vector2i(500, 250))


# ---------------------------------------------------------------------------
# Tree navigation
# ---------------------------------------------------------------------------

func _on_tree_item_activated() -> void:
	var selected := _error_tree.get_selected()
	if selected == null or plugin == null:
		return
	var file_path: String = selected.get_metadata(0)
	var line: int = selected.get_metadata(1)
	if file_path.is_empty():
		return

	# Parent items (file groups) have line == -1 — just toggle collapse
	if line < 1:
		selected.collapsed = not selected.collapsed
		return

	# Convert relative path to res:// if needed
	if not file_path.begins_with("res://"):
		file_path = "res://" + file_path

	plugin.goto_issue(file_path, line+1)


# ---------------------------------------------------------------------------
# Sorting helper
# ---------------------------------------------------------------------------

func _cmp_diagnostics(a: Dictionary, b: Dictionary) -> bool:
	var fa: String = a.get("file", "")
	var fb: String = b.get("file", "")
	if fa != fb:
		return fa < fb
	var la: int = a.get("span", {}).get("start_line", 0)
	var lb: int = b.get("span", {}).get("start_line", 0)
	return la < lb
