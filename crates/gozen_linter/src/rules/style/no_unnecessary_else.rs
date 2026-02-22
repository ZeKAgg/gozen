use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoUnnecessaryElse;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/noUnnecessaryElse",
    name: "noUnnecessaryElse",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Unnecessary `elif`/`else` after a body ending with `return`, `break`, or `continue`.",
    explanation: "When an `if` or `elif` body ends with `return`, `break`, or `continue`, subsequent `elif`/`else` branches are unnecessary because the earlier branch already exits. The code can be flattened for readability.",
};

impl Rule for NoUnnecessaryElse {
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

        walk_tree(root, source, |node, _src| {
            if node.kind() != "if_statement" {
                return;
            }
            check_if_statement(node, &mut diags);
        });

        diags
    }
}

/// Check an if_statement node for unnecessary elif/else branches.
fn check_if_statement(if_node: Node, diags: &mut Vec<Diagnostic>) {
    // An if_statement in tree-sitter-gdscript typically has children like:
    //   "if" condition body ["elif" condition body]* ["else" body]
    // We look for body nodes and check if they end with return/break/continue.
    // If a body ends with an exit statement and is followed by elif/else, flag it.

    let mut i = 0;
    let count = if_node.child_count();
    let mut prev_body_exits = false;

    while i < count {
        if let Some(child) = if_node.child(i) {
            let kind = child.kind();

            if crate::rules::is_block_node(kind) {
                prev_body_exits = body_ends_with_exit(child);
            } else if kind == "elif_clause" || kind == "elif" {
                if prev_body_exits {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "Unnecessary `elif` — previous branch already exits.".to_string(),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(child),
                        notes: vec![],
                        fix: None,
                    });
                }
                // Check the elif's own body for the next sibling
                prev_body_exits = has_exiting_body(child);
            } else if (kind == "else_clause" || kind == "else") && prev_body_exits {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Unnecessary `else` — previous branch already exits.".to_string(),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(child),
                    notes: vec![],
                    fix: None,
                });
            }
        }
        i += 1;
    }
}

/// Check if a body/block node's last named statement is return, break, or continue.
fn body_ends_with_exit(body: Node) -> bool {
    // Find the last named non-comment child
    let mut last_stmt = None;
    for i in 0..body.child_count() {
        if let Some(child) = body.child(i) {
            if child.is_named() && child.kind() != "comment" {
                last_stmt = Some(child);
            }
        }
    }
    if let Some(stmt) = last_stmt {
        return matches!(
            stmt.kind(),
            "return_statement" | "break_statement" | "continue_statement"
        );
    }
    false
}

/// Check if a elif_clause/elif node has a body child that ends with an exit.
fn has_exiting_body(clause: Node) -> bool {
    for i in 0..clause.child_count() {
        if let Some(child) = clause.child(i) {
            if crate::rules::is_block_node(child.kind()) {
                return body_ends_with_exit(child);
            }
        }
    }
    false
}
