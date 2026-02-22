use std::collections::HashSet;

use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoRepeatedGroupLookup;

const METADATA: RuleMetadata = RuleMetadata {
    id: "performance/noRepeatedGroupLookup",
    name: "noRepeatedGroupLookup",
    group: "performance",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Same group looked up multiple times in the same function.",
    explanation: "Calling get_tree().get_nodes_in_group() with the same group name multiple times in one function is wasteful. Cache the result in a local variable.",
};

impl Rule for NoRepeatedGroupLookup {
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
                check_function(node, src, &mut diags);
            }
        });
        diags
    }
}

fn check_function(func_node: Node, source: &str, diags: &mut Vec<Diagnostic>) {
    // Track: group_name -> first occurrence span
    let mut seen_groups: HashSet<String> = HashSet::new();

    walk_tree(func_node, source, |node, src| {
        if node.kind() != "call_expression" && node.kind() != "call" {
            return;
        }

        // Use the call name from the AST instead of text.contains() to avoid
        // matching inside strings or comments
        let call = gozen_parser::call_name(node, src);
        if call != "get_nodes_in_group" && call != "get_first_node_in_group" {
            return;
        }

        let text = node_text(node, src).trim();
        // Extract the group name from the arguments
        if let Some(group_name) = extract_group_name(text) {
            if !seen_groups.insert(group_name.clone()) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "Group \"{}\" is looked up multiple times in the same function.",
                        group_name
                    ),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(node),
                    notes: vec![Note {
                        message: "Cache the result in a local variable for better performance."
                            .to_string(),
                        span: None,
                    }],
                    fix: None,
                });
            }
        }
    });
}

/// Extract the group name string from a get_nodes_in_group("name") call text.
fn extract_group_name(text: &str) -> Option<String> {
    // Look for the pattern: get_nodes_in_group("..." or get_first_node_in_group("..."
    let patterns = ["get_nodes_in_group(", "get_first_node_in_group("];
    for pattern in &patterns {
        if let Some(pos) = text.find(pattern) {
            let after = &text[pos + pattern.len()..];
            let trimmed = after.trim();
            if let Some(quoted) = trimmed.strip_prefix('"') {
                // Find the closing quote
                if let Some(end) = quoted.find('"') {
                    return Some(quoted[..end].to_string());
                }
            }
        }
    }
    None
}
