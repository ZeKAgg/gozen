use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::Tree;

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

pub struct OneStatementPerLine;

impl ShaderRule for OneStatementPerLine {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/oneStatementPerLine",
            name: "oneStatementPerLine",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Multiple statements on one line.",
            explanation: "Each statement should be on its own line for readability. Avoid writing multiple statements separated by `;` on the same line.",
        }
    }

    fn check(&self, _tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut byte_offset = 0;

        for (row, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            // Skip empty lines, comments, and single-statement lines
            if trimmed.is_empty() || trimmed.starts_with("//") {
                byte_offset += line.len() + 1;
                continue;
            }

            // Skip for-loop lines: `for (int i = 0; i < 10; i++)` naturally has multiple semicolons
            if trimmed.starts_with("for(") || trimmed.starts_with("for (") {
                byte_offset += line.len() + 1;
                continue;
            }

            // Count semicolons outside of strings and comments
            let semi_count = count_semicolons(trimmed);
            if semi_count > 1 {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "Multiple statements on one line ({} semicolons).",
                        semi_count
                    ),
                    file_path: None,
                    rule_id: None,
                    span: Span {
                        start_byte: byte_offset,
                        end_byte: byte_offset + line.len(),
                        start_row: row,
                        start_col: 0,
                        end_row: row,
                        end_col: line.len(),
                    },
                    notes: Vec::new(),
                    fix: None,
                });
            }
            byte_offset += line.len() + 1;
        }

        diags
    }
}

fn count_semicolons(line: &str) -> usize {
    let mut count = 0;
    let mut in_string = false;
    let mut in_comment = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if in_comment {
            break;
        }
        if chars[i] == '"' && !in_comment {
            in_string = !in_string;
        } else if !in_string && i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            in_comment = true;
        } else if !in_string && !in_comment && chars[i] == ';' {
            count += 1;
        }
        i += 1;
    }
    count
}
