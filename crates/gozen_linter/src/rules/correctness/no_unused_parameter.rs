use std::collections::HashSet;

use gozen_diagnostics::{Diagnostic, Fix, Severity, TextEdit};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoUnusedParameter;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noUnusedParameter",
    name: "noUnusedParameter",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: true,
    description: "Function parameters that are never used.",
    explanation: "Unused parameters may indicate forgotten implementation or an incorrect function signature. Prefix with _ to indicate intentionally unused.",
};

/// Godot virtual methods whose parameters are part of the engine contract.
const GODOT_VIRTUAL: &[&str] = &[
    "_ready",
    "_process",
    "_physics_process",
    "_input",
    "_unhandled_input",
    "_unhandled_key_input",
    "_draw",
    "_integrate_forces",
    "_exit_tree",
    "_enter_tree",
    "_init",
    "_notification",
    "_get_configuration_warnings",
    "_get_property_list",
    "_set",
    "_get",
];

impl Rule for NoUnusedParameter {
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
            // Get function name — skip Godot virtual methods
            if let Some(name_node) = first_identifier_child(node) {
                let fn_name = node_text(name_node, src);
                if GODOT_VIRTUAL.contains(&fn_name) {
                    return;
                }
            }
            // Collect parameter names and spans
            let params = collect_parameters(node, src);
            if params.is_empty() {
                return;
            }
            // Collect all identifiers used in the function body
            let used = collect_body_identifiers(node, src);

            for (name, span) in params {
                if name.starts_with('_') {
                    continue; // Intentionally unused
                }
                if !used.contains(&name) {
                    let name_span = span;
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Parameter \"{}\" is never used.", name),
                        file_path: None,
                        rule_id: None,
                        span: name_span,
                        notes: vec![],
                        fix: Some(Fix {
                            description: format!("Prefix with _ to mark as unused: _{}", name),
                            is_safe: true,
                            changes: vec![TextEdit {
                                span: name_span,
                                new_text: format!("_{}", name),
                            }],
                        }),
                    });
                }
            }
        });
        diags
    }
}

fn collect_parameters(func_node: Node, source: &str) -> Vec<(String, gozen_diagnostics::Span)> {
    let mut params = Vec::new();
    for i in 0..func_node.child_count() {
        if let Some(child) = func_node.child(i) {
            let ck = child.kind();
            if ck == "parameters" || ck == "parameter_list" {
                for j in 0..child.child_count() {
                    if let Some(param) = child.child(j) {
                        if !param.is_named() {
                            continue;
                        }
                        // Parameter nodes: look for identifier child
                        if let Some(id_node) = first_identifier_child(param) {
                            let name = node_text(id_node, source).to_string();
                            params.push((name, span_from_node(id_node)));
                        } else if param.kind() == "identifier" {
                            let name = node_text(param, source).to_string();
                            params.push((name, span_from_node(param)));
                        }
                    }
                }
            }
        }
    }
    params
}

fn collect_body_identifiers(func_node: Node, source: &str) -> HashSet<String> {
    let mut identifiers = HashSet::new();
    // Find the body block of the function
    let mut body: Option<Node> = None;
    for i in 0..func_node.child_count() {
        if let Some(child) = func_node.child(i) {
            if crate::rules::is_block_node(child.kind()) {
                body = Some(child);
                break;
            }
        }
    }
    if let Some(body_node) = body {
        walk_tree(body_node, source, |n, src| {
            // Only collect identifiers that are actual code references,
            // not those inside string literals or comments
            if n.kind() == "identifier" {
                let in_string_or_comment = n.parent().is_some_and(|p| {
                    matches!(
                        p.kind(),
                        "string" | "string_literal" | "comment" | "line_comment" | "block_comment"
                    )
                });
                if !in_string_or_comment {
                    identifiers.insert(node_text(n, src).to_string());
                }
            }
        });
    }
    identifiers
}
