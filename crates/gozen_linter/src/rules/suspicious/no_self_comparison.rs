use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoSelfComparison;

const METADATA: RuleMetadata = RuleMetadata {
    id: "suspicious/noSelfComparison",
    name: "noSelfComparison",
    group: "suspicious",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Comparing a value to itself (`x == x`, `x != x`).",
    explanation: "Comparing a variable to itself is always true (for ==) or always false (for !=) and is likely a typo.",
};

impl Rule for NoSelfComparison {
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
            // Comparison expressions: ==, !=, <, >, <=, >=
            if k == "comparison_operator"
                || k == "binary_operator"
                || k == "comparison_expression"
                || k == "binary_expression"
            {
                let child_count = node.child_count();
                if child_count < 3 {
                    return;
                }
                // Find the comparison operator
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
                // Get left operand (first named child before operator)
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
                // Get right operand (first named child after operator)
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
                // Both must be identifiers with the same name
                if lhs.kind() != "identifier" || rhs.kind() != "identifier" {
                    return;
                }
                let lhs_name = node_text(lhs, src);
                let rhs_name = node_text(rhs, src);
                if lhs_name == rhs_name {
                    let op_text = if let Some(c) = node.child(op_idx) {
                        node_text(c, src)
                    } else {
                        "=="
                    };
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Comparing \"{}\" to itself with \"{}\" is always {}.",
                            lhs_name,
                            op_text,
                            if op_text == "==" { "true" } else { "false" }
                        ),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(node),
                        notes: vec![],
                        fix: None,
                    });
                }
            }
        });
        diags
    }
}
