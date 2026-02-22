use gozen_diagnostics::{Diagnostic, Fix, Severity, Span, TextEdit};
use gozen_parser::Tree;

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct ShaderCommentSpacing;

impl ShaderRule for ShaderCommentSpacing {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/commentSpacing",
            name: "commentSpacing",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: true,
            description: "Comments should have a space after `//`.",
            explanation: "Per the Godot Shaders Style Guide, use `// comment` instead of `//comment`. This improves readability.",
        }
    }

    fn check(&self, _tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut byte_offset = 0;

        for (row, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("//") {
                // Skip shebangs and empty comments
                if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('/') {
                    let col = line.len() - trimmed.len();
                    let start_byte = byte_offset + col;
                    let end_byte = start_byte + 2 + rest.len();

                    let span = Span {
                        start_byte,
                        end_byte,
                        start_row: row,
                        start_col: col,
                        end_row: row,
                        end_col: col + 2 + rest.len(),
                    };

                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "Missing space after `//` in comment.".to_string(),
                        file_path: None,
                        rule_id: None,
                        span,
                        notes: Vec::new(),
                        fix: Some(Fix {
                            description: "Add space after `//`".to_string(),
                            is_safe: true,
                            changes: vec![TextEdit {
                                span,
                                new_text: format!("// {}", rest),
                            }],
                        }),
                    });
                }
            }
            byte_offset += line.len() + 1; // +1 for newline
        }

        diags
    }
}
