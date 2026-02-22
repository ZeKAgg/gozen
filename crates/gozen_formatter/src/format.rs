use gozen_config::FormatterConfig;
use gozen_parser::Tree;

use crate::printer::Printer;
use crate::shader_printer::ShaderPrinter;
use crate::Span;

/// Apply end_of_line conversion. The formatter always produces LF internally.
/// If the config specifies "crlf", convert LF to CRLF as a final step.
fn apply_end_of_line(output: &str, config: &FormatterConfig) -> String {
    match config.end_of_line.as_str() {
        "crlf" | "CRLF" => {
            // First normalize to LF (in case of any stray CRLF), then convert to CRLF
            let lf_only = output.replace("\r\n", "\n");
            lf_only.replace('\n', "\r\n")
        }
        _ => output.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct TextChange {
    pub span: Span,
    pub old_text: String,
    pub new_text: String,
}

/// Format GDScript source. Returns the formatted string.
pub fn format(source: &str, tree: &Tree, config: &FormatterConfig) -> String {
    let mut printer = Printer::new(config);
    let output = printer.print(tree, source);
    apply_end_of_line(&output, config)
}

/// Check if source is already formatted.
pub fn is_formatted(source: &str, tree: &Tree, config: &FormatterConfig) -> bool {
    format(source, tree, config) == source
}

/// Return a diff of changes the formatter would make.
pub fn format_diff(source: &str, tree: &Tree, config: &FormatterConfig) -> Vec<TextChange> {
    let formatted = format(source, tree, config);
    if formatted == source {
        return Vec::new();
    }
    let line_count = source.lines().count().saturating_sub(1);
    vec![TextChange {
        span: Span {
            start_byte: 0,
            end_byte: source.len(),
            start_row: 0,
            start_col: 0,
            end_row: line_count,
            end_col: source.lines().last().map(|l| l.len()).unwrap_or(0),
        },
        old_text: source.to_string(),
        new_text: formatted,
    }]
}

/// Format GDShader source. Returns the formatted string.
pub fn format_shader(source: &str, tree: &Tree, config: &FormatterConfig) -> String {
    let mut printer = ShaderPrinter::new(config);
    let output = printer.print(tree, source);
    apply_end_of_line(&output, config)
}

/// Check if GDShader source is already formatted.
pub fn is_shader_formatted(source: &str, tree: &Tree, config: &FormatterConfig) -> bool {
    format_shader(source, tree, config) == source
}

#[cfg(test)]
mod tests {
    use super::*;
    use gozen_parser::{GDScriptParser, GDShaderParser};

    #[test]
    fn test_formatter_idempotent() {
        let config = FormatterConfig::default();
        let mut parser = GDScriptParser::new();
        let input = "extends Node\n\nvar x: int = 1\n";
        let tree = parser.parse(input).unwrap();
        let once = format(input, &tree, &config);
        let tree2 = parser.parse(&once).unwrap();
        let twice = format(&once, &tree2, &config);
        assert_eq!(once, twice, "Formatter must be idempotent");
    }

    /// Helper: assert idempotency for a given GDScript source snippet.
    /// Returns the first-pass formatted output for further inspection.
    fn assert_idempotent(label: &str, input: &str) -> String {
        let config = FormatterConfig::default();
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(input).expect("parse input");
        let once = format(input, &tree, &config);
        let tree2 = parser.parse(&once).expect("parse formatted output");
        let twice = format(&once, &tree2, &config);
        assert_eq!(
            once, twice,
            "Formatter NOT idempotent for '{}'.\n--- pass 1 ---\n{}\n--- pass 2 ---\n{}",
            label, once, twice
        );
        once
    }

    #[test]
    fn test_idempotent_complex_script() {
        let input = r#"extends Node

@export var health: int = 100
@onready var sprite = $Sprite2D
var speed: float = 5.0

signal health_changed(new_health: int)

const MAX_HEALTH = 200

enum State { IDLE, RUNNING, JUMPING }

func _ready():
	print("hello")
	var x = 10

func _process(delta):
	if health > 0:
		health -= 1
	for i in range(10):
		print(i)

func public_method():
	pass

func _private_method():
	return 42
"#;
        assert_idempotent("complex_script", input);
    }

    #[test]
    fn test_idempotent_expressions() {
        let input = r#"extends Node

func _ready():
	var arr = [1, 2, 3]
	var dict = {"a": 1, "b": 2}
	print("hello", "world")
	var result = some_func(a, b, c)
"#;
        assert_idempotent("expressions", input);
    }

    #[test]
    fn test_idempotent_control_flow() {
        let input = r#"extends Node

func _ready():
	if true:
		pass
	elif false:
		pass
	else:
		pass
	for i in range(10):
		print(i)
	while true:
		break
	match x:
		1:
			print("one")
		_:
			print("other")
"#;
        assert_idempotent("control_flow", input);
    }

    #[test]
    fn test_idempotent_crlf() {
        let input = "extends Node\r\n\r\nvar x: int = 1\r\n\r\nfunc _ready():\r\n\tpass\r\n";
        assert_idempotent("crlf", input);
    }

    #[test]
    fn test_idempotent_decorated_definitions() {
        let input = r#"extends Node

@export var health: int = 100
@export var speed: float = 5.0
@onready var sprite = $Sprite2D
@onready var label = $Label
var normal_var = 10

func _ready():
	pass
"#;
        assert_idempotent("decorated_definitions", input);
    }

    /// Verify the formatter doesn't DROP content (functions, statements, etc.)
    #[test]
    fn test_formatter_preserves_functions() {
        let config = FormatterConfig::default();
        let mut parser = GDScriptParser::new();
        let input = "extends Node\n\nfunc _ready():\n\tprint(\"hello\")\n\tvar x = 10\n";
        let tree = parser.parse(input).expect("parse");
        let result = format(input, &tree, &config);
        assert!(
            result.contains("func _ready"),
            "Formatter must preserve function definitions. Got:\n{}",
            result
        );
        assert!(
            result.contains("print"),
            "Formatter must preserve function body. Got:\n{}",
            result
        );
    }

    /// Asserts idempotency for all fixture-like inputs.
    #[test]
    fn test_idempotent_fixture_inputs() {
        let inputs = [
            ("basic_function", "extends   Node\n\nvar   health:int=100\nvar  speed :  float  = 5.0\n\nfunc   _ready(  ):\n    print(  \"hello\"  )\n    var x=   10\n\nfunc _process(  delta  ):\n    health+=1\n"),
            ("decorated_vars", "extends   Node\n\n@export var   health:int=100\n@export var  speed :  float  = 5.0\n@onready  var  sprite  =  $Sprite2D\nvar  normal_var  =  10\n\nfunc  _ready(  ):\n\tpass\n"),
            ("already_formatted", "extends Node\n\nfunc _ready():\n\tpass\n"),
            ("indentation_spaces", "extends Node\nfunc _ready():\n\tprint(\"tab indented\")\n"),
            ("trailing_commas", "var a = [1, 2, 3]\nvar d = { \"x\": 1, \"y\": 2 }\n"),
        ];

        for (label, input) in inputs {
            assert_idempotent(label, input);
        }
    }

    /// Style-guide-shaped samples that should stay parse-stable and idempotent.
    #[test]
    fn test_idempotent_styleguide_shaped_inputs() {
        let inputs = [
            (
                "styleguide_tabs_indentation",
                "extends Node\nfunc _ready():\n    var health:int=100\n    print(\"ok\")\n",
            ),
            (
                "styleguide_function_declaration_spacing",
                "extends Node\n\nfunc   move_player( delta:float, speed:int ):\n\tpass\n",
            ),
            (
                "styleguide_trailing_comma_behavior",
                "func _ready():\n\tprint([1,2,3,])\n",
            ),
        ];

        for (label, input) in inputs {
            let output = assert_idempotent(label, input);
            let mut parser = GDScriptParser::new();
            let reparsed = parser.parse(&output);
            assert!(
                reparsed.is_some(),
                "Output must remain parseable for {}",
                label
            );
        }
    }

    #[test]
    fn test_styleguide_no_space_before_call_parens() {
        let config = FormatterConfig::default();
        let mut parser = GDScriptParser::new();
        let input = "func _ready():\n\tprint (\"x\")\n\tobj.method (\"x\")\n";
        let tree = parser.parse(input).unwrap();
        let output = format(input, &tree, &config);
        let expected = "func _ready():\n\tprint(\"x\")\n\tobj.method(\"x\")\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_styleguide_expression_operator_spacing() {
        let config = FormatterConfig::default();
        let mut parser = GDScriptParser::new();
        let input = "func _ready():\n\tposition.x=5\n\ta+=1\n\tif a==b:\n\t\tpass\n";
        let tree = parser.parse(input).unwrap();
        let output = format(input, &tree, &config);
        let expected = "func _ready():\n\tposition.x = 5\n\ta += 1\n\tif a == b:\n\t\tpass\n";
        assert_eq!(output, expected);
    }

    /// Test idempotency for files with comments (the most critical fix).
    #[test]
    fn test_idempotent_comments() {
        let input = r#"# NOTE this class could be cleaned up
# maybe a BattleUnitSpawner component
class_name BattleHandler
extends Node

signal battle_ended

const BATTLE_UNIT = preload("res://scenes/battle_unit/battle_unit.tscn")

@export var game_state: GameState

# NOTE testing code
func _input(event: InputEvent) -> void:
	if event.is_action_pressed("test1"):
		get_tree().call_group("player_units", "queue_free")
"#;
        let result = assert_idempotent("comments", input);
        // Ensure comments are preserved as separate lines
        assert!(
            result.contains("# NOTE this class"),
            "Comment must be preserved. Got:\n{}",
            result
        );
        assert!(
            result.contains("class_name BattleHandler"),
            "class_name must be on its own line. Got:\n{}",
            result
        );
    }

    /// Test idempotency for functions with return type annotations.
    #[test]
    fn test_idempotent_return_types() {
        let input = r#"extends Node

func _ready() -> void:
	pass

func get_health() -> int:
	return 100

func spawn_scene(parent: Node = owner) -> Node:
	var new_scene := scene.instantiate()
	parent.add_child(new_scene)
	return new_scene
"#;
        let result = assert_idempotent("return_types", input);
        // Ensure return types are preserved
        assert!(
            result.contains("-> void"),
            "Return type '-> void' must be preserved. Got:\n{}",
            result
        );
        assert!(
            result.contains("-> int"),
            "Return type '-> int' must be preserved. Got:\n{}",
            result
        );
        assert!(
            result.contains("-> Node"),
            "Return type '-> Node' must be preserved. Got:\n{}",
            result
        );
    }

    /// Test idempotency for multi-line variable declarations with setget.
    #[test]
    fn test_idempotent_setget() {
        let input = r#"class_name DetectRange
extends Area2D

@export var col_shape: CollisionShape2D
@export var base_range_size: float
@export var stats: UnitStats:
	set(value):
		stats = value
		collision_layer = 4 * (stats.team + 1)

		var shape := CircleShape2D.new()
		shape.radius = base_range_size * stats.attack_range
		col_shape.shape = shape
"#;
        let result = assert_idempotent("setget", input);
        // Ensure the setter body is preserved across multiple lines
        assert!(
            result.contains("set(value)"),
            "Setter must be preserved. Got:\n{}",
            result
        );
        assert!(
            result.contains("shape.radius"),
            "Setter body must be preserved. Got:\n{}",
            result
        );
    }

    /// Test idempotency for files with inline comments in function bodies.
    #[test]
    fn test_idempotent_inline_comments() {
        let input = r#"extends Node

func _start_chasing() -> void:
	var chase_state := ChaseState.new(actor)
	chase_state.stuck.connect(_on_chase_state_stuck, CONNECT_ONE_SHOT)
	fsm.change_state(chase_state)
	# NOTE
	# this was previously at the end of the chase_state's enter() method
	# BUT we need this here
	chase_state.chase()
"#;
        let result = assert_idempotent("inline_comments", input);
        assert!(
            result.contains("# NOTE"),
            "Inline comment must be preserved. Got:\n{}",
            result
        );
    }

    /// Test idempotency for @tool and other standalone annotations.
    #[test]
    fn test_idempotent_tool_annotation() {
        let input = r#"@tool
class_name CustomSkin
extends Sprite2D

@export var stats: UnitStats

func set_stats(value: UnitStats) -> void:
	stats = value
"#;
        assert_idempotent("tool_annotation", input);
    }

    /// Test idempotency for CRLF files with return types and comments.
    #[test]
    fn test_idempotent_crlf_with_return_types() {
        let input = "extends Node\r\n\r\n# A comment\r\nfunc _ready() -> void:\r\n\tpass\r\n";
        let result = assert_idempotent("crlf_return_types", input);
        assert!(
            result.contains("-> void"),
            "Return type must be preserved in CRLF files. Got:\n{}",
            result
        );
    }

    /// Test idempotency for real-world state.gd pattern.
    #[test]
    fn test_idempotent_state_pattern() {
        let input = r#"class_name State
extends RefCounted

var actor: Node


func _init(new_actor: Node) -> void:
	actor = new_actor


func physics_process(_delta: float) -> void:
	pass


func process(_delta: float) -> void:
	pass


func enter() -> void:
	pass


func exit() -> void:
	pass
"#;
        let result = assert_idempotent("state_pattern", input);
        assert!(
            result.contains("-> void"),
            "Return type must be preserved. Got:\n{}",
            result
        );
        assert!(
            result.contains("func _init"),
            "All functions must be preserved. Got:\n{}",
            result
        );
        assert!(
            result.contains("func exit"),
            "Last function must be preserved. Got:\n{}",
            result
        );
    }

    /// Test idempotency for a match statement pattern.
    #[test]
    fn test_idempotent_match_pattern() {
        let input = r#"extends Node

func _on_game_state_changed() -> void:
	match game_state.current_phase:
		GameState.Phase.PREPARATION:
			_clean_up_fight()
		GameState.Phase.BATTLE:
			_prepare_fight()
"#;
        assert_idempotent("match_pattern", input);
    }

    // ── Shader formatter tests ────────────────────────────────────────────

    /// Helper: assert idempotency for a given GDShader source snippet.
    fn assert_shader_idempotent(label: &str, input: &str) -> String {
        let config = FormatterConfig::default();
        let mut parser = GDShaderParser::new();
        let tree = parser.parse(input).expect("parse shader input");
        let once = format_shader(input, &tree, &config);
        let tree2 = parser.parse(&once).expect("parse formatted shader output");
        let twice = format_shader(&once, &tree2, &config);
        assert_eq!(
            once, twice,
            "Shader formatter NOT idempotent for '{}'.\n--- pass 1 ---\n{}\n--- pass 2 ---\n{}",
            label, once, twice
        );
        once
    }

    #[test]
    fn test_shader_idempotent_basic() {
        let input = r#"shader_type canvas_item;

uniform vec4 color : source_color = vec4(1.0);

void fragment() {
	COLOR = color;
}
"#;
        assert_shader_idempotent("basic_shader", input);
    }

    #[test]
    fn test_shader_idempotent_function() {
        let input = r#"shader_type spatial;
render_mode unshaded;

uniform sampler2D albedo_texture;

void vertex() {
	VERTEX.y += sin(TIME);
}

void fragment() {
	vec4 tex = texture(albedo_texture, UV);
	ALBEDO = tex.rgb;
	ALPHA = tex.a;
}
"#;
        assert_shader_idempotent("shader_function", input);
    }

    #[test]
    fn test_shader_idempotent_for_loop() {
        let input = r#"shader_type canvas_item;

uniform int line_thickness : hint_range(0, 10) = 1;
uniform sampler2D screen_texture : hint_screen_texture;

void fragment() {
	vec2 size = SCREEN_PIXEL_SIZE * float(line_thickness);
	float alpha = 0.0;
	for (float i = -size.x; i <= size.x; i += SCREEN_PIXEL_SIZE.x) {
		for (float j = -size.y; j <= size.y; j += SCREEN_PIXEL_SIZE.y) {
			alpha += texture(screen_texture, SCREEN_UV + vec2(i, j)).a;
		}
	}
	COLOR.a = min(alpha, 1.0);
}
"#;
        assert_shader_idempotent("shader_for_loop", input);
    }

    #[test]
    fn test_shader_idempotent_struct() {
        let input = r#"shader_type spatial;

struct Light {
	vec3 position;
	vec3 color;
	float intensity;
};

void fragment() {
	ALBEDO = vec3(1.0);
}
"#;
        assert_shader_idempotent("shader_struct", input);
    }

    #[test]
    fn test_shader_idempotent_uniforms() {
        let input = r#"shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_back;

group_uniforms albedo;
uniform vec4 albedo_color : source_color = vec4(1.0);
uniform sampler2D albedo_texture : source_color;

group_uniforms;

uniform float roughness : hint_range(0.0, 1.0) = 0.5;
uniform float metallic : hint_range(0.0, 1.0) = 0.0;

varying vec3 world_normal;

void vertex() {
	world_normal = NORMAL;
}

void fragment() {
	ALBEDO = albedo_color.rgb * texture(albedo_texture, UV).rgb;
	ROUGHNESS = roughness;
	METALLIC = metallic;
}
"#;
        assert_shader_idempotent("shader_uniforms", input);
    }

    #[test]
    fn test_shader_idempotent_if_else() {
        let input = r#"shader_type spatial;

void fragment() {
	if (UV.x > 0.5) {
		ALBEDO = vec3(1.0, 0.0, 0.0);
	} else {
		ALBEDO = vec3(0.0, 0.0, 1.0);
	}
}
"#;
        assert_shader_idempotent("shader_if_else", input);
    }

    /// Dump the AST structure for a given GDScript snippet (debug helper).
    #[test]
    #[ignore]
    fn dump_ast_structure() {
        let mut parser = GDScriptParser::new();
        let input = r#"extends Node

# A comment
@export var health: int = 100
var speed: float = 5.0

func _ready() -> void:
	print("hello")
	var x = 10
	# inline comment
	x = 20
	return x
	pass
"#;
        let tree = parser.parse(input).expect("parse input");
        let root = tree.root_node();
        fn dump(node: gozen_parser::Node, source: &str, depth: usize) {
            let indent = "  ".repeat(depth);
            let text = gozen_parser::node_text(node, source);
            let text_preview: String = text.chars().take(60).collect();
            let text_preview = text_preview.replace('\n', "\\n").replace('\t', "\\t");
            eprintln!(
                "{}{} [{}] named={} children={} \"{}\"",
                indent,
                node.kind(),
                node.start_position().row,
                node.is_named(),
                node.child_count(),
                text_preview
            );
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    dump(child, source, depth + 1);
                }
            }
        }
        dump(root, input, 0);
    }
}
