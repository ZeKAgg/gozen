use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, walk_tree, Tree};
use std::collections::{HashMap, HashSet};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

/// Built-in entry points that are always "used" by the Godot engine.
const ENTRY_POINTS: &[&str] = &[
    "vertex", "fragment", "light", "start", "process", "sky", "fog",
];

pub struct UnusedFunction;

impl ShaderRule for UnusedFunction {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/unusedFunction",
            name: "unusedFunction",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Helper function declared but never called.",
            explanation:
                "Dead code in shaders wastes compilation time. Remove unused helper functions.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut functions: HashMap<String, gozen_parser::Node> = HashMap::new();

        // Collect all function declarations
        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "function_declaration" {
                    if let Some(name) = extract_func_name(child, source) {
                        // Skip entry points
                        if !ENTRY_POINTS.contains(&name.as_str()) {
                            functions.insert(name, child);
                        }
                    }
                }
            }
        }

        if functions.is_empty() {
            return Vec::new();
        }

        // Scan for call_expr references
        let mut called: HashSet<String> = HashSet::new();
        walk_tree(root, source, |node, _src| {
            if node.kind() == "call_expr" {
                // First child is typically the function name
                if let Some(name_node) = node.child(0) {
                    let name = node_text(name_node, source).trim().to_string();
                    if functions.contains_key(&name) {
                        called.insert(name);
                    }
                }
            }
        });

        let mut diags = Vec::new();
        for (name, node) in &functions {
            if !called.contains(name) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Function `{}` is declared but never called.", name),
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
        diags
    }
}

fn extract_func_name(node: gozen_parser::Node, source: &str) -> Option<String> {
    // function_declaration has children: return_type, name (identifier), parameter_list, block
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return Some(node_text(child, source).trim().to_string());
            }
        }
    }
    // Fallback: extract from text
    let text = node_text(node, source);
    let parts: Vec<&str> = text.split('(').next()?.split_whitespace().collect();
    if parts.len() >= 2 {
        Some(parts.last()?.to_string())
    } else {
        None
    }
}
