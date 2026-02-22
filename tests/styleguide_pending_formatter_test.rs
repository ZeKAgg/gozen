//! Pending formatter conformance tests against Godot's GDScript style guide.
//! These are intentionally ignored until the formatter behavior is implemented.

use gozen_config::FormatterConfig;
use gozen_formatter::format;
use gozen_parser::GDScriptParser;

fn format_gd(input: &str) -> String {
    let config = FormatterConfig::default();
    let mut parser = GDScriptParser::new();
    let tree = parser.parse(input).expect("input should parse");
    format(input, &tree, &config)
}

#[test]
fn pending_remove_space_before_call_parens() {
    let input = "func _ready():\n\tprint (\"x\")\n";
    let expected = "func _ready():\n\tprint(\"x\")\n";
    assert_eq!(format_gd(input), expected);
}

#[test]
fn pending_operator_spacing_expression_statement() {
    let input = "func _ready():\n\tposition.x=5\n";
    let expected = "func _ready():\n\tposition.x = 5\n";
    assert_eq!(format_gd(input), expected);
}

#[test]
fn pending_blank_line_between_top_level_functions() {
    let input = "func a():\n\tpass\nfunc b():\n\tpass\n";
    let expected = "func a():\n\tpass\n\nfunc b():\n\tpass\n";
    assert_eq!(format_gd(input), expected);
}

#[test]
fn pending_dictionary_brace_inner_spacing_style() {
    let input = "func _ready():\n\tvar d = {\"key\": 1}\n";
    let expected = "func _ready():\n\tvar d = { \"key\": 1 }\n";
    assert_eq!(format_gd(input), expected);
}

#[test]
fn pending_continuation_indentation_wrapped_condition() {
    let input = "func _ready():\n\tif (\n\t\ttrue\n\t\tand false\n\t):\n\t\tpass\n";
    let expected = "func _ready():\n\tif (\n\t\ttrue\n\t\tand false\n\t):\n\t\tpass\n";
    assert_eq!(format_gd(input), expected);
}
