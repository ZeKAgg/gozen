use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoShadowedVariable;

const METADATA: RuleMetadata = RuleMetadata {
    id: "suspicious/noShadowedVariable",
    name: "noShadowedVariable",
    group: "suspicious",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Inner scope variable with same name as outer scope.",
    explanation: "Shadowing can make code harder to read. Use a different name.",
};

impl Rule for NoShadowedVariable {
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
        // Start with an initial scope for file-level declarations
        let mut scope: Vec<std::collections::HashSet<String>> =
            vec![std::collections::HashSet::new()];
        walk_with_scope(root, source, &mut scope, &mut diags);
        diags
    }
}

fn walk_with_scope(
    node: Node,
    source: &str,
    scope: &mut Vec<std::collections::HashSet<String>>,
    diags: &mut Vec<Diagnostic>,
) {
    if node.kind() == "variable_statement" {
        if let Some(name_node) = first_identifier_child(node) {
            let name = node_text(name_node, source).to_string();
            if !name.starts_with('_') {
                // Only check the immediate parent scope for shadowing (not all ancestors)
                // to avoid overly aggressive warnings on deeply nested code
                if scope.len() >= 2 {
                    let parent_scope = &scope[scope.len() - 2];
                    if parent_scope.contains(&name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "Variable \"{}\" shadows a variable from outer scope.",
                                name
                            ),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(name_node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
                // scope is always non-empty (initialized with file-level scope)
                if let Some(current) = scope.last_mut() {
                    current.insert(name);
                }
            }
        }
    } else if crate::rules::is_block_node(node.kind()) {
        scope.push(std::collections::HashSet::new());
        for i in 0..node.child_count() {
            if let Some(c) = node.child(i) {
                if c.is_named() {
                    walk_with_scope(c, source, scope, diags);
                }
            }
        }
        scope.pop();
        return;
    }
    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) {
            if c.is_named() {
                walk_with_scope(c, source, scope, diags);
            }
        }
    }
}
