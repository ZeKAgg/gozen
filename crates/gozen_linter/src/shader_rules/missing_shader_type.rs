use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::Tree;

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct MissingShaderType;

impl ShaderRule for MissingShaderType {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/missingShaderType",
            name: "missingShaderType",
            group: "shader",
            default_severity: Severity::Error,
            has_fix: false,
            description: "File lacks a `shader_type` declaration.",
            explanation: "Every GDShader file must begin with a `shader_type` declaration (e.g. `shader_type spatial;`). Without it, Godot cannot determine the shader pipeline.",
        }
    }

    fn check(&self, tree: &Tree, _source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut found = false;
        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "shader_type_declaration" {
                    found = true;
                    break;
                }
            }
        }
        if found {
            return Vec::new();
        }
        vec![Diagnostic {
            severity: Severity::Error,
            message: "Missing `shader_type` declaration. Add `shader_type spatial;`, `shader_type canvas_item;`, or `shader_type particles;` at the top of the file.".to_string(),
            file_path: None,
            rule_id: None,
            span: Span { start_byte: 0, end_byte: 0, start_row: 0, start_col: 0, end_row: 0, end_col: 0 },
            notes: Vec::new(),
            fix: None,
        }]
    }
}
