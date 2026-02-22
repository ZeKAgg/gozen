use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{walk_tree, Tree};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct AvoidDiscard;

impl ShaderRule for AvoidDiscard {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/avoidDiscard",
            name: "avoidDiscard",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Use of `discard` has a GPU performance cost.",
            explanation: "The `discard` keyword prevents early-Z optimizations and can significantly reduce performance, especially on mobile GPUs. Consider using alpha blending instead where possible.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();

        walk_tree(root, source, |node, _src| {
            // Only match the discard_statement node, not its child tokens
            if node.kind() == "discard_statement" {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message:
                        "`discard` can be expensive on some GPUs. Consider alpha blending instead."
                            .to_string(),
                    file_path: None,
                    rule_id: None,
                    span: Span {
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        start_row: node.start_position().row,
                        start_col: node.start_position().column,
                        end_row: node.end_position().row,
                        end_col: node.end_position().column,
                    },
                    notes: Vec::new(),
                    fix: None,
                });
            }
        });

        diags
    }
}
