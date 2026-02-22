use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{
    call_name, first_identifier_child, node_text, span_from_node, walk_tree, Node, Tree,
};

use crate::rule::{Rule, RuleMetadata};

pub struct NoExpensiveProcess;

const EXPENSIVE_CALLS: &[&str] = &[
    "get_nodes_in_group",
    "load",
    "find_child",
    "get_node",
    "get_node_or_null",
    "find_node",
    "instantiate",
    "preload",
    "get_children",
];

const METADATA: RuleMetadata = RuleMetadata {
    id: "performance/noExpensiveProcess",
    name: "noExpensiveProcess",
    group: "performance",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Expensive operations inside _process or _physics_process.",
    explanation: "Cache results of get_node, get_nodes_in_group, find_child, etc. with @onready or in _ready() instead of calling every frame.",
};

impl Rule for NoExpensiveProcess {
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
            if node.kind() == "function_definition" {
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src);
                    if name == "_process" || name == "_physics_process" {
                        check_body_for_expensive(node, src, &mut diags);
                    }
                }
            }
        });
        diags
    }
}

fn check_body_for_expensive(node: Node, source: &str, diags: &mut Vec<Diagnostic>) {
    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) {
            walk_tree(c, source, |n, src| {
                if n.kind() == "call_expression" || n.kind() == "call" {
                    let name = call_name(n, src);
                    if EXPENSIVE_CALLS.contains(&name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Consider caching the result of \"{}\".", name),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(n),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
            });
        }
    }
}
