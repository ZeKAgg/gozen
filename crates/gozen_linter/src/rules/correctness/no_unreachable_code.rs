use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{span_from_node, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoUnreachableCode;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noUnreachableCode",
    name: "noUnreachableCode",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Statements after return, break, or continue in the same block.",
    explanation: "Code after a return/break/continue is never executed and is likely a mistake.",
};

impl Rule for NoUnreachableCode {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        tree: &Tree,
        _source: &str,
        _context: Option<&crate::context::LintContext>,
    ) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();
        check_block(root, &mut diags);
        diags
    }
}

fn check_block(node: Node, diags: &mut Vec<Diagnostic>) {
    let kind = node.kind();
    if crate::rules::is_block_node(kind) {
        let mut seen_terminal = false;
        for i in 0..node.child_count() {
            let child = match node.child(i) {
                Some(c) => c,
                None => continue,
            };
            if !child.is_named() {
                continue;
            }
            let k = child.kind();
            if k == "return_statement" || k == "break_statement" || k == "continue_statement" {
                seen_terminal = true;
            } else if seen_terminal && !k.is_empty() {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Unreachable code after return/break/continue.".to_string(),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(child),
                    notes: vec![],
                    fix: None,
                });
            }
            check_block(child, diags);
        }
    } else {
        for i in 0..node.child_count() {
            if let Some(c) = node.child(i) {
                check_block(c, diags);
            }
        }
    }
}
