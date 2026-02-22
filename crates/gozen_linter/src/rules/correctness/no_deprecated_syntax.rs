use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::Tree;

use crate::rule::{Rule, RuleMetadata};

pub struct NoDeprecatedSyntax;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noDeprecatedSyntax",
    name: "noDeprecatedSyntax",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Godot 3 syntax like `setget` that is removed in Godot 4.",
    explanation: "The `setget` keyword was removed in Godot 4. Use property syntax with `set` and `get` instead.",
};

impl Rule for NoDeprecatedSyntax {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        _tree: &Tree,
        source: &str,
        _context: Option<&crate::context::LintContext>,
    ) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let mut byte_offset: usize = 0;
        let src_bytes = source.as_bytes();

        for (line_idx, line) in source.lines().enumerate() {
            let line_bytes = line.len();
            let trimmed = line.trim();

            // Check for `setget` keyword (Godot 3 syntax)
            if let Some(pos) = trimmed.find("setget") {
                // Make sure it's a standalone keyword, not part of a string or comment
                let before = if pos > 0 {
                    trimmed.chars().nth(pos - 1)
                } else {
                    Some(' ')
                };
                let after = trimmed.chars().nth(pos + 6);
                let is_word_boundary = before.is_none_or(|c| !c.is_alphanumeric() && c != '_')
                    && after.is_none_or(|c| !c.is_alphanumeric() && c != '_');

                // Not inside a comment or string — use proper string state tracking
                let in_comment_or_string = is_inside_comment_or_string(trimmed, pos);

                if is_word_boundary && !in_comment_or_string {
                    // Calculate the actual position of the keyword within the original line
                    let leading_ws = line.len() - trimmed.len();
                    let keyword_start_col = leading_ws + pos;
                    let keyword_end_col = keyword_start_col + 6; // "setget".len()
                    let keyword_start_byte = byte_offset + keyword_start_col;
                    let keyword_end_byte = byte_offset + keyword_end_col;
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: "The `setget` keyword was removed in Godot 4. Use property syntax with `set` and `get`.".to_string(),
                        file_path: None,
                        rule_id: None,
                        span: Span {
                            start_byte: keyword_start_byte,
                            end_byte: keyword_end_byte,
                            start_row: line_idx,
                            start_col: keyword_start_col,
                            end_row: line_idx,
                            end_col: keyword_end_col,
                        },
                        notes: vec![],
                        fix: None,
                    });
                }
            }

            // Check for old `yield` usage (replaced by `await` in Godot 4)
            if let Some(pos) = trimmed.find("yield") {
                let before = if pos > 0 {
                    trimmed.chars().nth(pos - 1)
                } else {
                    Some(' ')
                };
                let after = trimmed.chars().nth(pos + 5);
                let is_word_boundary = before.is_none_or(|c| !c.is_alphanumeric() && c != '_')
                    && after.is_none_or(|c| c == '(' || (!c.is_alphanumeric() && c != '_'));

                let in_comment_or_string = is_inside_comment_or_string(trimmed, pos);

                if is_word_boundary && !in_comment_or_string {
                    // Calculate the actual position of the keyword within the original line
                    let leading_ws = line.len() - trimmed.len();
                    let keyword_start_col = leading_ws + pos;
                    let keyword_end_col = keyword_start_col + 5; // "yield".len()
                    let keyword_start_byte = byte_offset + keyword_start_col;
                    let keyword_end_byte = byte_offset + keyword_end_col;
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: "The `yield` keyword was removed in Godot 4. Use `await` instead."
                            .to_string(),
                        file_path: None,
                        rule_id: None,
                        span: Span {
                            start_byte: keyword_start_byte,
                            end_byte: keyword_end_byte,
                            start_row: line_idx,
                            start_col: keyword_start_col,
                            end_row: line_idx,
                            end_col: keyword_end_col,
                        },
                        notes: vec![],
                        fix: None,
                    });
                }
            }

            // Advance byte_offset past the line content and its line ending
            byte_offset += line_bytes;
            // Account for actual line ending (\r\n or \n)
            if byte_offset < src_bytes.len() {
                if src_bytes[byte_offset] == b'\r' {
                    byte_offset += 1;
                }
                if byte_offset < src_bytes.len() && src_bytes[byte_offset] == b'\n' {
                    byte_offset += 1;
                }
            }
        }
        diags
    }
}

/// Check if a byte position within a line is inside a comment or string literal.
/// Properly handles escaped quotes, unlike simple quote counting.
fn is_inside_comment_or_string(line: &str, target_pos: usize) -> bool {
    let mut in_string = false;
    let mut string_char = '"';
    let mut prev = '\0';
    for (i, c) in line.char_indices() {
        if i >= target_pos {
            return in_string; // Reached the target — return whether we're in a string
        }
        if in_string {
            if c == string_char && prev != '\\' {
                in_string = false;
            }
        } else if c == '#' {
            return true; // Everything after # is a comment
        } else if c == '"' || c == '\'' {
            in_string = true;
            string_char = c;
        }
        prev = c;
    }
    in_string
}
