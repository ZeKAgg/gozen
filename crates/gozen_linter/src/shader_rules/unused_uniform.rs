use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, walk_tree, Tree};
use std::collections::HashMap;

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct UnusedUniform;

impl ShaderRule for UnusedUniform {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/unusedUniform",
            name: "unusedUniform",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Uniform declared but never referenced in shader code.",
            explanation:
                "Unused uniforms waste GPU memory. Remove them or use them in your shader code.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut uniforms: HashMap<String, gozen_parser::Node> = HashMap::new();

        // Collect all uniform declarations
        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "uniform_declaration" {
                    let text = node_text(child, source);
                    if let Some(name) = extract_uniform_name(text) {
                        uniforms.insert(name, child);
                    }
                }
            }
        }

        if uniforms.is_empty() {
            return Vec::new();
        }

        // Scan all identifiers for references, but skip identifiers that are
        // children of uniform_declaration nodes (the declaration itself).
        let mut referenced: std::collections::HashSet<String> = std::collections::HashSet::new();
        walk_tree(root, source, |node, _src| {
            if node.kind() == "ident_expr" || node.kind() == "identifier" {
                // Skip if this identifier is inside a uniform_declaration (self-reference)
                let mut parent = node.parent();
                while let Some(p) = parent {
                    if p.kind() == "uniform_declaration" {
                        return;
                    }
                    parent = p.parent();
                }
                let name = node_text(node, source).trim().to_string();
                if uniforms.contains_key(&name) {
                    referenced.insert(name);
                }
            }
        });

        let mut diags = Vec::new();
        for (name, node) in &uniforms {
            if !referenced.contains(name) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Uniform `{}` is declared but never used.", name),
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

fn extract_uniform_name(text: &str) -> Option<String> {
    // "uniform type name ..." — name is after the type
    let trimmed = text.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    // uniform <type> <name> [: hint] [= default] ;
    if parts.len() >= 3 && parts[0] == "uniform" {
        // The name might have trailing characters like : or ;
        let name = parts[2].trim_end_matches([':', ';', '=']);
        Some(name.to_string())
    } else {
        None
    }
}
