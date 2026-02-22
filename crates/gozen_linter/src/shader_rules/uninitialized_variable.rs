use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, walk_tree, Tree};
use std::collections::HashSet;

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct UninitializedVariable;

impl ShaderRule for UninitializedVariable {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/uninitializedVariable",
            name: "uninitializedVariable",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Local variable used before assignment.",
            explanation: "Using a local variable before it has been assigned a value may lead to undefined behaviour. Assign a value at declaration.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        // Simple heuristic: find var_declaration without initializer, then check if
        // the variable name is used before any assignment_statement targeting it.
        // This is a conservative check that only works within a single function.
        let root = tree.root_node();
        let mut diags = Vec::new();

        walk_tree(root, source, |node, _src| {
            if node.kind() == "function_declaration" {
                diags.extend(check_function(node, source));
            }
        });

        diags
    }
}

fn check_function(func: gozen_parser::Node, source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut uninitialized: HashSet<String> = HashSet::new();
    let mut initialized: HashSet<String> = HashSet::new();

    // Walk children in order to track variable state
    walk_tree(func, source, |node, _src| {
        let kind = node.kind();

        if kind == "var_declaration" {
            // Check if it has an initializer (= expr)
            let text = node_text(node, source);
            let name = extract_var_name(text);
            if let Some(name) = name {
                if text.contains('=') {
                    initialized.insert(name);
                } else {
                    uninitialized.insert(name);
                }
            }
        } else if kind == "assignment_statement" {
            let text = node_text(node, source);
            if let Some(lhs) = text.split('=').next() {
                let lhs = lhs.trim().to_string();
                if uninitialized.contains(&lhs) {
                    uninitialized.remove(&lhs);
                    initialized.insert(lhs);
                }
            }
        } else if kind == "ident_expr" {
            let name = node_text(node, source).trim().to_string();
            if uninitialized.contains(&name) && !initialized.contains(&name) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Variable `{}` may be used before initialization.", name),
                    file_path: None,
                    rule_id: None,
                    span: Span {
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        start_row: node.start_position().row,
                        start_col: node.start_position().column,
                        end_row: node.end_position().row,
                        end_col: node.end_position().column,
                    },
                    notes: Vec::new(),
                    fix: None,
                });
            }
        }
    });

    diags
}

fn extract_var_name(text: &str) -> Option<String> {
    // Handles: "type name", "type name = expr", "const type name = expr",
    // "lowp float name", etc.
    let trimmed = text.trim();
    // Remove trailing ;
    let trimmed = trimmed.trim_end_matches(';').trim();
    // Split on = for initializer
    let decl = trimmed.split('=').next()?.trim();
    // The variable name is the last token in the declaration part
    // (after any qualifiers and the type)
    let parts: Vec<&str> = decl.split_whitespace().collect();
    if parts.len() >= 2 {
        // The name is always the last part: "const float x" -> "x", "float x" -> "x"
        Some(parts.last()?.to_string())
    } else {
        None
    }
}
