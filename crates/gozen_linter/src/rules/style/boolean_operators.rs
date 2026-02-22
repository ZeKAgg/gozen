use gozen_diagnostics::{Diagnostic, Fix, Severity, TextEdit};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct BooleanOperators;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/booleanOperators",
    name: "booleanOperators",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: true,
    description: "Prefer `and`/`or`/`not` over `&&`/`||`/`!`.",
    explanation: "The GDScript style guide recommends plain English boolean operators for readability: use `and` instead of `&&`, `or` instead of `||`, and `not` instead of `!`.",
};

impl Rule for BooleanOperators {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        tree: &Tree,
        source: &str,
        _context: Option<&crate::context::LintContext>,
    ) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();
        walk_tree(root, source, |node, _src| {
            let text = node_text(node, source);
            // Only match the exact operator tokens, not parent expression nodes.
            // This prevents replacing `a && b` (the whole expression) with `and`.
            let (bad_op, good_op) = if text == "&&" {
                ("&&", "and")
            } else if text == "||" {
                ("||", "or")
            } else if text == "!" && node.kind() != "identifier" {
                // Avoid matching != operator: the `!` token for negation
                // stands alone, not as part of `!=`
                ("!", "not ")
            } else {
                return;
            };

            let span = span_from_node(node);
            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: format!(
                    "Use \"{}\" instead of \"{}\" for boolean operations.",
                    good_op.trim(),
                    bad_op
                ),
                file_path: None,
                rule_id: None,
                span,
                notes: vec![],
                fix: Some(Fix {
                    description: format!("Replace \"{}\" with \"{}\"", bad_op, good_op.trim()),
                    is_safe: true,
                    changes: vec![TextEdit {
                        span,
                        new_text: good_op.to_string(),
                    }],
                }),
            });
        });
        diags
    }
}
