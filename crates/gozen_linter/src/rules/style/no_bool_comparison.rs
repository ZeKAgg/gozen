use gozen_diagnostics::{Diagnostic, Fix, Severity, TextEdit};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoBoolComparison;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/noBoolComparison",
    name: "noBoolComparison",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: true,
    description: "Avoid explicit comparison to `true` or `false`.",
    explanation:
        "Comparing to true/false is redundant. Use the value directly or `not` for negation.",
};

impl Rule for NoBoolComparison {
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

        walk_tree(root, source, |node, src| {
            let k = node.kind();
            if k != "comparison_operator"
                && k != "binary_operator"
                && k != "comparison_expression"
                && k != "binary_expression"
            {
                return;
            }
            let child_count = node.child_count();
            if child_count < 3 {
                return;
            }
            // Find the operator
            let mut op_idx = None;
            for i in 0..child_count {
                if let Some(c) = node.child(i) {
                    let text = node_text(c, src);
                    if text == "==" || text == "!=" {
                        op_idx = Some(i);
                        break;
                    }
                }
            }
            let op_idx = match op_idx {
                Some(i) => i,
                None => return,
            };
            let op_text = node.child(op_idx).map(|c| node_text(c, src)).unwrap_or("");

            // Get left and right operands
            let lhs = {
                let mut found = None;
                for i in 0..op_idx {
                    if let Some(c) = node.child(i) {
                        if c.is_named() {
                            found = Some(c);
                            break;
                        }
                    }
                }
                match found {
                    Some(n) => n,
                    None => return,
                }
            };
            let rhs = {
                let mut found = None;
                for i in (op_idx + 1)..child_count {
                    if let Some(c) = node.child(i) {
                        if c.is_named() {
                            found = Some(c);
                            break;
                        }
                    }
                }
                match found {
                    Some(n) => n,
                    None => return,
                }
            };

            let lhs_text = node_text(lhs, src);
            let rhs_text = node_text(rhs, src);
            let span = span_from_node(node);

            // Check for comparisons to true/false
            let (value_text, is_true) = if rhs_text == "true" {
                (lhs_text, true)
            } else if rhs_text == "false" {
                (lhs_text, false)
            } else if lhs_text == "true" {
                (rhs_text, true)
            } else if lhs_text == "false" {
                (rhs_text, false)
            } else {
                return;
            };

            // Determine replacement:
            // x == true  -> x
            // x == false -> not x
            // x != true  -> not x
            // x != false -> x
            let (message, replacement) = if op_text == "==" && is_true {
                (
                    format!(
                        "Redundant comparison `{} == true`. Use `{}` directly.",
                        value_text, value_text
                    ),
                    value_text.to_string(),
                )
            } else if op_text == "==" && !is_true {
                (
                    format!(
                        "Redundant comparison `{} == false`. Use `not {}` instead.",
                        value_text, value_text
                    ),
                    format!("not {}", value_text),
                )
            } else if op_text == "!=" && is_true {
                (
                    format!(
                        "Redundant comparison `{} != true`. Use `not {}` instead.",
                        value_text, value_text
                    ),
                    format!("not {}", value_text),
                )
            } else {
                // != false
                (
                    format!(
                        "Redundant comparison `{} != false`. Use `{}` directly.",
                        value_text, value_text
                    ),
                    value_text.to_string(),
                )
            };

            diags.push(Diagnostic {
                severity: Severity::Warning,
                message,
                file_path: None,
                rule_id: None,
                span,
                notes: vec![],
                fix: Some(Fix {
                    description: format!("Replace with `{}`", replacement),
                    is_safe: true,
                    changes: vec![TextEdit {
                        span,
                        new_text: replacement,
                    }],
                }),
            });
        });
        diags
    }
}
