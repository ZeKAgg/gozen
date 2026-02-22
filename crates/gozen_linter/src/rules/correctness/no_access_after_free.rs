use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{call_name, node_text, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoAccessAfterFree;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noAccessAfterFree",
    name: "noAccessAfterFree",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Accessing node properties or methods after queue_free().",
    explanation: "After calling queue_free(), the node is scheduled for deletion. Accessing its properties or methods afterward can cause errors.",
};

impl Rule for NoAccessAfterFree {
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
        // Check each block/function for queue_free() followed by statements
        walk_tree(root, source, |node, _src| {
            let k = node.kind();
            if crate::rules::is_block_node(k) {
                check_block_for_access_after_free(node, source, &mut diags);
            }
        });
        diags
    }
}

fn check_block_for_access_after_free(block: Node, source: &str, diags: &mut Vec<Diagnostic>) {
    let mut seen_queue_free = false;
    for i in 0..block.child_count() {
        let child = match block.child(i) {
            Some(c) => c,
            None => continue,
        };
        if !child.is_named() {
            continue;
        }
        if seen_queue_free {
            let text = node_text(child, source);
            // Skip return statements, comments, and pass — those are safe
            if child.kind() == "return_statement"
                || child.kind() == "comment"
                || child.kind() == "pass_statement"
            {
                continue;
            }
            // Only flag statements that actually access the freed node:
            // - Member access (contains `.` suggesting property/method access)
            // - Explicit self references
            // - Method calls on the freed object
            let accesses_self =
                text.contains("self.") || text.contains("self[") || is_self_word(text);
            let has_member_access = text.contains('.')
                && !text.starts_with("print")
                && !text.starts_with("emit_signal")
                && !text.starts_with("var ");
            if accesses_self || has_member_access {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Accessing node after queue_free() — node is scheduled for deletion."
                        .to_string(),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(child),
                    notes: vec![],
                    fix: None,
                });
            }
        }
        // Check if this statement contains a queue_free() call
        if contains_queue_free(child, source) {
            seen_queue_free = true;
        }
    }
}

/// Check if `text` starts with "self" as a whole word (not "selfish", "self_destruct", etc.)
fn is_self_word(text: &str) -> bool {
    if !text.starts_with("self") {
        return false;
    }
    // Check that the character after "self" is not alphanumeric or underscore
    text.as_bytes()
        .get(4)
        .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_')
}

fn contains_queue_free(node: Node, source: &str) -> bool {
    let mut found = false;
    walk_tree(node, source, |n, src| {
        if found {
            return;
        }
        if n.kind() == "call_expression" || n.kind() == "call" {
            let name = call_name(n, src);
            if name == "queue_free" {
                found = true;
            }
        }
    });
    found
}
