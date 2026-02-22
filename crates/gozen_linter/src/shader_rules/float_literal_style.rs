use gozen_diagnostics::{Diagnostic, Fix, Severity, Span, TextEdit};
use gozen_parser::{node_text, walk_tree, Tree};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct FloatLiteralStyle;

impl ShaderRule for FloatLiteralStyle {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/floatLiteralStyle",
            name: "floatLiteralStyle",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: true,
            description: "Float literals should have digits on both sides of the decimal point.",
            explanation: "Per the Godot Shaders Style Guide, use `0.5` instead of `.5` and `5.0` instead of `5.`. This improves readability.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();

        walk_tree(root, source, |node, _src| {
            // Look for float literals (primitive_expr with float content)
            if node.kind() == "primitive_expr" || node.kind() == "float_literal" {
                let text = node_text(node, source).trim().to_string();
                if let Some(fixed) = fix_float(&text) {
                    let span = Span {
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        start_row: node.start_position().row,
                        start_col: node.start_position().column,
                        end_row: node.end_position().row,
                        end_col: node.end_position().column,
                    };
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Float literal `{}` should be written as `{}`.",
                            text, fixed
                        ),
                        file_path: None,
                        rule_id: None,
                        span,
                        notes: Vec::new(),
                        fix: Some(Fix {
                            description: format!("Rewrite as `{}`", fixed),
                            is_safe: true,
                            changes: vec![TextEdit {
                                span,
                                new_text: fixed,
                            }],
                        }),
                    });
                }
            }
        });

        diags
    }
}

fn fix_float(text: &str) -> Option<String> {
    let text = text.trim();
    // Only operate on numeric text with a decimal point
    if !text.contains('.') {
        return None;
    }
    // Strip trailing f suffix if present
    let base = text.trim_end_matches('f');
    let has_f = text.ends_with('f');

    if base.starts_with('.') {
        // .5 -> 0.5
        let fixed = format!("0{}", base);
        return Some(if has_f { format!("{}f", fixed) } else { fixed });
    }

    if base.ends_with('.') {
        // 5. -> 5.0
        let fixed = format!("{}0", base);
        return Some(if has_f { format!("{}f", fixed) } else { fixed });
    }

    None
}
