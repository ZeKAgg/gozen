use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, Tree};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

const VALID_TYPES: &[&str] = &["spatial", "canvas_item", "particles", "sky", "fog"];

pub struct InvalidShaderType;

impl ShaderRule for InvalidShaderType {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/invalidShaderType",
            name: "invalidShaderType",
            group: "shader",
            default_severity: Severity::Error,
            has_fix: false,
            description: "Unknown shader type in `shader_type` declaration.",
            explanation: "The `shader_type` must be one of: `spatial`, `canvas_item`, `particles`, `sky`, or `fog`. Other values are invalid.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();
        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "shader_type_declaration" {
                    let text = node_text(child, source);
                    // Extract the type name: "shader_type <name>;"
                    let trimmed = text.trim().trim_end_matches(';').trim();
                    if let Some(type_name) = trimmed.strip_prefix("shader_type") {
                        let type_name = type_name.trim();
                        if !VALID_TYPES.contains(&type_name) {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                message: format!(
                                    "Invalid shader type `{}`. Expected one of: spatial, canvas_item, particles, sky, fog.",
                                    type_name
                                ),
                                file_path: None,
                                rule_id: None,
                                span: Span {
                                    start_byte: child.start_byte(),
                                    end_byte: child.end_byte(),
                                    start_row: child.start_position().row,
                                    start_col: child.start_position().column,
                                    end_row: child.end_position().row,
                                    end_col: child.end_position().column,
                                },
                                notes: Vec::new(),
                                fix: None,
                            });
                        }
                    }
                }
            }
        }
        diags
    }
}
