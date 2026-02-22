use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoSelfAssignment;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noSelfAssignment",
    name: "noSelfAssignment",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Variable assigned to itself (`x = x`).",
    explanation:
        "Assigning a variable to itself has no effect and is likely a typo or copy-paste error.",
};

impl Rule for NoSelfAssignment {
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
            // Assignment expressions: look for `x = x` pattern
            if k == "assignment_statement"
                || k == "assignment"
                || k == "expression_statement"
                || k == "assignment_expression"
            {
                // An assignment node typically has left, operator, right children
                // We need at least 3 children: left, "=", right
                let child_count = node.child_count();
                if child_count < 3 {
                    return;
                }
                // Find the "=" operator among children
                let mut eq_idx = None;
                for i in 0..child_count {
                    if let Some(c) = node.child(i) {
                        let text = node_text(c, src);
                        if text == "=" {
                            eq_idx = Some(i);
                            break;
                        }
                    }
                }
                let eq_idx = match eq_idx {
                    Some(i) => i,
                    None => return,
                };
                // Get the left-hand side (first named child before =)
                let lhs = {
                    let mut found = None;
                    for i in 0..eq_idx {
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
                // Get the right-hand side (first named child after =)
                let rhs = {
                    let mut found = None;
                    for i in (eq_idx + 1)..child_count {
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
                // Both sides must be simple identifiers
                if lhs.kind() != "identifier" || rhs.kind() != "identifier" {
                    return;
                }
                let lhs_name = node_text(lhs, src);
                let rhs_name = node_text(rhs, src);
                if lhs_name == rhs_name {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("Variable \"{}\" is assigned to itself.", lhs_name),
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
