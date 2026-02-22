use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{
    call_name, first_identifier_child, node_text, span_from_node, walk_tree, Node, Tree,
};

use crate::rule::{Rule, RuleMetadata};

pub struct NoAddChildInProcess;

const METADATA: RuleMetadata = RuleMetadata {
    id: "performance/noAddChildInProcess",
    name: "noAddChildInProcess",
    group: "performance",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "add_child() inside _process() or _physics_process().",
    explanation: "Adding children inside per-frame callbacks can cause issues with scene tree modification during processing. Use call_deferred(\"add_child\", node) or add_child.call_deferred(node) instead.",
};

impl Rule for NoAddChildInProcess {
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
            if node.kind() != "function_definition" {
                return;
            }
            if let Some(name_node) = first_identifier_child(node) {
                let fn_name = node_text(name_node, src);
                if fn_name == "_process" || fn_name == "_physics_process" {
                    check_body_for_add_child(node, src, &mut diags);
                }
            }
        });
        diags
    }
}

fn check_body_for_add_child(func_node: Node, source: &str, diags: &mut Vec<Diagnostic>) {
    for i in 0..func_node.child_count() {
        if let Some(c) = func_node.child(i) {
            walk_tree(c, source, |n, src| {
                if n.kind() == "call_expression" || n.kind() == "call" {
                    let name = call_name(n, src);
                    if name == "add_child" || name == "add_sibling" || name == "remove_child" {
                        // Check if this call is chained through call_deferred by examining
                        // the parent node. If the parent is also a call with name "call_deferred",
                        // or if the call is `add_child.call_deferred(...)`, skip it.
                        if is_deferred_call(n, src) {
                            return;
                        }
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "\"{}\" inside a per-frame callback. Consider using call_deferred().",
                                name
                            ),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(n),
                            notes: vec![Note {
                                message: format!(
                                    "Use {}.call_deferred(node) to avoid scene tree modification during processing.",
                                    name
                                ),
                                span: None,
                            }],
                            fix: None,
                        });
                    }
                    // Also check call_deferred("add_child", ...) pattern — that's fine, skip it
                    // (no action needed for call_deferred calls)
                }
            });
        }
    }
}

/// Check if a call node is actually invoked via call_deferred (e.g., `add_child.call_deferred(...)`).
fn is_deferred_call(node: Node, source: &str) -> bool {
    // Check parent: if the parent is an attribute/member access with "call_deferred",
    // then this is already deferred.
    if let Some(parent) = node.parent() {
        let pk = parent.kind();
        if pk == "call_expression" || pk == "call" {
            let parent_name = call_name(parent, source);
            if parent_name == "call_deferred" {
                return true;
            }
        }
        // Check if any sibling/child of this call's parent is "call_deferred"
        if pk == "attribute" || pk == "member_expression" || pk == "get_attribute" {
            if let Some(grandparent) = parent.parent() {
                let gp_name = call_name(grandparent, source);
                if gp_name == "call_deferred" {
                    return true;
                }
            }
        }
    }
    false
}
