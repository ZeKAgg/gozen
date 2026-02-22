use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, walk_tree, Tree};
use std::collections::HashMap;

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct UnusedVarying;

impl ShaderRule for UnusedVarying {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/unusedVarying",
            name: "unusedVarying",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Varying declared but never read in the shader.",
            explanation:
                "Unused varyings consume interpolator slots. Remove them if they are not needed.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut varyings: HashMap<String, gozen_parser::Node> = HashMap::new();

        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "varying_declaration" {
                    let text = node_text(child, source);
                    if let Some(name) = extract_varying_name(text) {
                        varyings.insert(name, child);
                    }
                }
            }
        }

        if varyings.is_empty() {
            return Vec::new();
        }

        let mut referenced: std::collections::HashSet<String> = std::collections::HashSet::new();
        walk_tree(root, source, |node, _src| {
            if node.kind() == "ident_expr" || node.kind() == "identifier" {
                // Skip if this identifier is inside a varying_declaration (self-reference)
                let mut parent = node.parent();
                while let Some(p) = parent {
                    if p.kind() == "varying_declaration" {
                        return;
                    }
                    parent = p.parent();
                }
                let name = node_text(node, source).trim().to_string();
                if varyings.contains_key(&name) {
                    referenced.insert(name);
                }
            }
        });

        let mut diags = Vec::new();
        for (name, node) in &varyings {
            if !referenced.contains(name) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Varying `{}` is declared but never used.", name),
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

fn extract_varying_name(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    // varying <type> <name>;
    if parts.len() >= 3 && parts[0] == "varying" {
        let name = parts[2].trim_end_matches(';');
        Some(name.to_string())
    } else {
        None
    }
}
