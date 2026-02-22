use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct SuperReadyFirst;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/superReadyFirst",
    name: "superReadyFirst",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "super._ready() or super() should be the first statement in _ready().",
    explanation: "Calling super._ready() first ensures the parent class initialization completes before custom setup. Putting it later can cause subtle ordering bugs.",
};

impl Rule for SuperReadyFirst {
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
            // Check if this is _ready()
            if let Some(name_node) = first_identifier_child(node) {
                let fn_name = node_text(name_node, src);
                if fn_name != "_ready" {
                    return;
                }
            } else {
                return;
            }
            // Find the body block
            let body = match find_body(node) {
                Some(b) => b,
                None => return,
            };
            // Look for super() or super._ready() call
            let super_info = find_super_call(body, src);
            if let Some((super_span, is_first)) = super_info {
                if !is_first {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "super._ready() should be the first statement in _ready()."
                            .to_string(),
                        file_path: None,
                        rule_id: None,
                        span: super_span,
                        notes: vec![],
                        fix: None,
                    });
                }
            }
        });
        diags
    }
}

fn find_body(func_node: Node) -> Option<Node> {
    for i in 0..func_node.child_count() {
        if let Some(child) = func_node.child(i) {
            let k = child.kind();
            if crate::rules::is_block_node(k) {
                return Some(child);
            }
        }
    }
    None
}

fn find_super_call(body: Node, source: &str) -> Option<(gozen_diagnostics::Span, bool)> {
    let mut first_statement_seen = false;
    for i in 0..body.child_count() {
        let child = match body.child(i) {
            Some(c) => c,
            None => continue,
        };
        if !child.is_named() {
            continue;
        }
        // Use AST traversal to find actual super calls instead of string matching
        if contains_super_call(child, source) {
            return Some((span_from_node(child), !first_statement_seen));
        }
        first_statement_seen = true;
    }
    None
}

/// Check if a node contains a call to super() or super._ready() using AST traversal.
fn contains_super_call(node: Node, source: &str) -> bool {
    let mut found = false;
    walk_tree(node, source, |n, _src| {
        if found {
            return;
        }
        let k = n.kind();
        // Look for call expressions where the callee is `super` or `super._ready`
        if k == "call_expression" || k == "call" {
            // Check if any child is "super" keyword
            for i in 0..n.child_count() {
                if let Some(c) = n.child(i) {
                    let ck = c.kind();
                    // Direct super() call
                    if ck == "super" {
                        found = true;
                        return;
                    }
                    // super._ready() — attribute access on super
                    if ck == "attribute" || ck == "member_expression" || ck == "get_attribute" {
                        for j in 0..c.child_count() {
                            if let Some(gc) = c.child(j) {
                                if gc.kind() == "super" {
                                    found = true;
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    found
}
