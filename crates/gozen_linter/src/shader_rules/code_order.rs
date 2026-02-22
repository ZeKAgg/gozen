use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::Tree;

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

/// Expected top-level declaration order per Godot Shaders Style Guide.
const ORDER: &[&str] = &[
    "shader_type_declaration",
    "render_mode_declaration",
    "include_declaration",
    "const_declaration",
    "uniform_declaration",
    "group_uniforms_declaration",
    "varying_declaration",
    "struct_declaration",
    "function_declaration",
];

fn order_index(kind: &str) -> usize {
    ORDER.iter().position(|k| *k == kind).unwrap_or(usize::MAX)
}

pub struct CodeOrder;

impl ShaderRule for CodeOrder {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/codeOrder",
            name: "codeOrder",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Top-level declarations should follow recommended order.",
            explanation: "The Godot Shaders Style Guide recommends this order: shader_type, render_mode, includes, constants, uniforms, group uniforms, varyings, structs, then functions.",
        }
    }

    fn check(&self, tree: &Tree, _source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();
        let mut max_order_seen: usize = 0;

        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if !child.is_named() {
                    continue;
                }
                let kind = child.kind();
                let idx = order_index(kind);
                if idx == usize::MAX {
                    continue;
                }
                if idx < max_order_seen {
                    let expected_before = ORDER.get(max_order_seen).unwrap_or(&"unknown");
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "`{}` should appear before `{}` declarations.",
                            kind.replace('_', " "),
                            expected_before.replace('_', " "),
                        ),
                        file_path: None,
                        rule_id: None,
                        span: Span {
                            start_byte: child.start_byte(),
                            end_byte: child.end_byte(),
                            start_row: child.start_position().row,
                            start_col: child.start_position().column,
                            end_row: child.end_position().row,
                            end_col: child.end_position().column,
                        },
                        notes: Vec::new(),
                        fix: None,
                    });
                } else {
                    max_order_seen = idx;
                }
            }
        }
        diags
    }
}
