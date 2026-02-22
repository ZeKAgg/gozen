use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoStringConcatLoop;

const METADATA: RuleMetadata = RuleMetadata {
    id: "performance/noStringConcatLoop",
    name: "noStringConcatLoop",
    group: "performance",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "String concatenation with += inside loops.",
    explanation: "String concatenation in loops creates a new string each iteration, resulting in O(n^2) performance. Use Array.append() and \"\".join() instead.",
};

impl Rule for NoStringConcatLoop {
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
            if k == "for_statement" || k == "while_statement" || k == "for_in_statement" {
                check_loop_body_for_concat(node, src, &mut diags);
            }
        });
        diags
    }
}

fn check_loop_body_for_concat(loop_node: Node, source: &str, diags: &mut Vec<Diagnostic>) {
    walk_tree(loop_node, source, |node, src| {
        let k = node.kind();
        // Look for += operator on strings
        if k == "assignment_statement"
            || k == "assignment"
            || k == "augmented_assignment"
            || k == "expression_statement"
        {
            let text = node_text(node, src);
            if !text.contains("+=") {
                return;
            }
            // Split on += and check the RHS specifically
            let parts: Vec<&str> = text.splitn(2, "+=").collect();
            if parts.len() != 2 {
                return;
            }
            let rhs = parts[1].trim();
            // Only flag when the RHS is clearly a string:
            // - Starts with a string literal ("..." or '...')
            // - Is a standalone str() call (not part of a longer expression like `int + str(x)`)
            let rhs_is_string = rhs.starts_with('"')
                || rhs.starts_with('\'')
                || rhs.starts_with("str(")
                || rhs.starts_with("\"%s\"")
                || rhs.starts_with("String(");

            if rhs_is_string {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "String concatenation with += in a loop is O(n^2).".to_string(),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(node),
                    notes: vec![Note {
                        message: "Use Array.append() and \"\".join() instead.".to_string(),
                        span: None,
                    }],
                    fix: None,
                });
            }
        }
    });
}
