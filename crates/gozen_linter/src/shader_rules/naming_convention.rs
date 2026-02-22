use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, walk_tree, Tree};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct ShaderNamingConvention;

impl ShaderRule for ShaderNamingConvention {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/namingConvention",
            name: "namingConvention",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Checks naming conventions: snake_case for functions/variables, CONSTANT_CASE for constants.",
            explanation: "Following the Godot Shaders Style Guide, use snake_case for functions and variables, and SCREAMING_SNAKE_CASE for constants.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();

        walk_tree(root, source, |node, _src| {
            let kind = node.kind();
            match kind {
                "const_declaration" => {
                    let text = node_text(node, source);
                    if let Some(name) = extract_const_name(text) {
                        if !is_screaming_snake_case(&name) {
                            diags.push(make_diag(
                                node,
                                format!("Constant `{}` should use SCREAMING_SNAKE_CASE.", name),
                            ));
                        }
                    }
                }
                "function_declaration" => {
                    if let Some(name) = extract_func_name_from_decl(node, source) {
                        // Skip built-in entry points
                        if !is_entry_point(&name) && !is_snake_case(&name) {
                            diags.push(make_diag(
                                node,
                                format!("Function `{}` should use snake_case.", name),
                            ));
                        }
                    }
                }
                "uniform_declaration" | "varying_declaration" => {
                    let text = node_text(node, source);
                    if let Some(name) = extract_decl_name(text) {
                        if !is_snake_case(&name) {
                            diags.push(make_diag(
                                node,
                                format!("`{}` should use snake_case.", name),
                            ));
                        }
                    }
                }
                _ => {}
            }
        });

        diags
    }
}

fn make_diag(node: gozen_parser::Node, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message,
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
    }
}

fn is_snake_case(name: &str) -> bool {
    // Allow leading underscore, then lowercase/digits/underscores
    let name = name.strip_prefix('_').unwrap_or(name);
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn is_screaming_snake_case(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn is_entry_point(name: &str) -> bool {
    matches!(
        name,
        "vertex" | "fragment" | "light" | "start" | "process" | "sky" | "fog"
    )
}

fn extract_const_name(text: &str) -> Option<String> {
    // "const NAME : type = value;"
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "const" {
        let name = parts[1].trim_end_matches(':').trim_end_matches('=');
        Some(name.to_string())
    } else {
        None
    }
}

fn extract_func_name_from_decl(node: gozen_parser::Node, source: &str) -> Option<String> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return Some(node_text(child, source).trim().to_string());
            }
        }
    }
    let text = node_text(node, source);
    let parts: Vec<&str> = text.split('(').next()?.split_whitespace().collect();
    parts.last().map(|s| s.to_string())
}

fn extract_decl_name(text: &str) -> Option<String> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() >= 3 {
        let name = parts[2].trim_end_matches([':', ';', '=']);
        Some(name.to_string())
    } else {
        None
    }
}
