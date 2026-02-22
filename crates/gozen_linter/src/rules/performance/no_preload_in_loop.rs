use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{call_name, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoPreloadInLoop;

const METADATA: RuleMetadata = RuleMetadata {
    id: "performance/noPreloadInLoop",
    name: "noPreloadInLoop",
    group: "performance",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "preload() or load() inside loops.",
    explanation: "Calling preload() or load() inside a loop reloads the resource each iteration. Cache the result in a constant or variable outside the loop.",
};

impl Rule for NoPreloadInLoop {
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
                check_loop_for_preload(node, src, &mut diags);
            }
        });
        diags
    }
}

fn check_loop_for_preload(loop_node: Node, source: &str, diags: &mut Vec<Diagnostic>) {
    walk_tree(loop_node, source, |node, src| {
        if node.kind() == "call_expression" || node.kind() == "call" {
            let name = call_name(node, src);
            if name == "preload" || name == "load" {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "\"{}\" inside a loop. Cache the result outside the loop.",
                        name
                    ),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(node),
                    notes: vec![Note {
                        message: "Move to a const or @onready var for better performance."
                            .to_string(),
                        span: None,
                    }],
                    fix: None,
                });
            }
        }
    });
}
