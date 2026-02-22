use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

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
];

pub struct NamingConvention;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/namingConvention",
    name: "namingConvention",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Godot naming: snake_case for functions/vars/signals, PascalCase for classes/enums, CONSTANT_CASE for constants/enum members.",
    explanation: "Follow Godot style: functions, variables, and signals use snake_case; classes and enum names use PascalCase; constants and enum members use UPPER_SNAKE_CASE.",
};

impl Rule for NamingConvention {
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
            if k == "function_definition" {
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src);
                    // Allow _-prefixed private/virtual functions (snake_case with leading _)
                    let name_to_check = name.strip_prefix('_').unwrap_or(name);
                    if !snake_case(name_to_check) && !GODOT_VIRTUAL.contains(&name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Function name \"{}\" should be snake_case.", name),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(name_node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
            } else if k == "class_name_statement" {
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src);
                    if !pascal_case(name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Class name \"{}\" should be PascalCase.", name),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(name_node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
            } else if k == "variable_statement" {
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src);
                    if !snake_case(name) && !name.starts_with('_') {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Variable name \"{}\" should be snake_case.", name),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(name_node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
            } else if k == "const_statement" || k == "constant_definition" {
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src);
                    if !upper_snake_case(name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "Constant name \"{}\" should be UPPER_SNAKE_CASE.",
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
            } else if k == "signal_statement" || k == "signal_declaration" {
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src);
                    if !snake_case(name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Signal name \"{}\" should be snake_case.", name),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(name_node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
            } else if k == "enum_definition" || k == "enum_statement" {
                // Check enum name (PascalCase)
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src);
                    if !pascal_case(name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Enum name \"{}\" should be PascalCase.", name),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(name_node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
                // Check enum members (CONSTANT_CASE)
                check_enum_members(node, src, &mut diags);
            }
        });
        diags
    }
}

fn check_enum_members(node: gozen_parser::Node, source: &str, diags: &mut Vec<Diagnostic>) {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let ck = child.kind();
            // Enum members may appear as enumerator, enum_value, identifier, etc.
            if ck == "enumerator" || ck == "enum_value" || ck == "enum_member" {
                if let Some(name_node) = first_identifier_child(child) {
                    let name = node_text(name_node, source);
                    if !upper_snake_case(name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Enum member \"{}\" should be CONSTANT_CASE.", name),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(name_node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
            } else if ck == "enum_body" || ck == "body" {
                // Recurse into the enum body
                check_enum_members(child, source, diags);
            }
        }
    }
}

fn snake_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars()
        .all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn pascal_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_uppercase() {
        return false;
    }
    // Single uppercase letter is valid PascalCase (e.g., class A)
    let rest: String = chars.collect();
    if rest.is_empty() {
        return true;
    }
    // Must have at least one lowercase letter and be all alphanumeric
    rest.chars().all(|c| c.is_alphanumeric()) && rest.chars().any(|c| c.is_ascii_lowercase())
}

fn upper_snake_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars()
        .all(|c| c == '_' || c.is_ascii_uppercase() || c.is_ascii_digit())
        && s.chars().any(|c| c.is_ascii_uppercase())
}
