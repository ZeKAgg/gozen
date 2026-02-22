use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::Tree;

use crate::rule::{Rule, RuleMetadata};

pub struct CommentSpacing;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/commentSpacing",
    name: "commentSpacing",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Comments should start with a space after `#`.",
    explanation: "The GDScript style guide requires regular comments (`#`) and doc comments (`##`) to start with a space. Commented-out code (`#print(...)`) should not. The `#region`/`#endregion` markers are excluded.",
};

impl Rule for CommentSpacing {
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

        /// Advance byte_offset past the current line's content and its actual line ending.
        fn advance_past_line(byte_offset: &mut usize, line_bytes: usize, src_bytes: &[u8]) {
            *byte_offset += line_bytes;
            if *byte_offset < src_bytes.len() {
                if src_bytes[*byte_offset] == b'\r' {
                    *byte_offset += 1;
                }
                if *byte_offset < src_bytes.len() && src_bytes[*byte_offset] == b'\n' {
                    *byte_offset += 1;
                }
            }
        }

        for (line_idx, line) in source.lines().enumerate() {
            let line_bytes = line.len();
            let trimmed = line.trim();
            // Find the comment portion of the line
            let comment_start = if trimmed.starts_with('#') {
                trimmed
            } else if let Some(pos) = find_inline_comment(trimmed) {
                &trimmed[pos..]
            } else {
                advance_past_line(&mut byte_offset, line_bytes, src_bytes);
                continue;
            };

            // Skip #region / #endregion (Godot code region markers, no space required)
            if comment_start.starts_with("#region") || comment_start.starts_with("#endregion") {
                advance_past_line(&mut byte_offset, line_bytes, src_bytes);
                continue;
            }

            let line_span = Span {
                start_byte: byte_offset,
                end_byte: byte_offset + line_bytes,
                start_row: line_idx,
                start_col: 0,
                end_row: line_idx,
                end_col: line_bytes,
            };

            // Doc comments: ## should be followed by a space (unless just ##)
            if let Some(rest) = comment_start.strip_prefix("##") {
                if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('#') {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "Doc comment should have a space after `##`.".to_string(),
                        file_path: None,
                        rule_id: None,
                        span: line_span,
                        notes: vec![],
                        fix: None,
                    });
                }
                advance_past_line(&mut byte_offset, line_bytes, src_bytes);
                continue;
            }

            // Regular comment: # should be followed by a space
            if comment_start.starts_with('#') && !comment_start.starts_with("##") {
                let rest = &comment_start[1..];
                if rest.is_empty() {
                    // Bare `#` is fine
                    advance_past_line(&mut byte_offset, line_bytes, src_bytes);
                    continue;
                }
                if rest.starts_with(' ') {
                    // Correct: `# comment`
                    advance_past_line(&mut byte_offset, line_bytes, src_bytes);
                    continue;
                }
                // No space after #. This is bad style for text comments.
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Comment should have a space after `#`.".to_string(),
                    file_path: None,
                    rule_id: None,
                    span: line_span,
                    notes: vec![],
                    fix: None,
                });
            }
            advance_past_line(&mut byte_offset, line_bytes, src_bytes);
        }
        diags
    }
}

/// Find the start position of an inline comment (# not inside a string).
/// Returns the byte offset within the given string slice.
fn find_inline_comment(line: &str) -> Option<usize> {
    let mut in_string = false;
    let mut string_char = '"';
    let mut prev = '\0';
    for (i, c) in line.char_indices() {
        if in_string {
            if c == string_char && prev != '\\' {
                in_string = false;
            }
        } else if c == '"' || c == '\'' {
            in_string = true;
            string_char = c;
        } else if c == '#' {
            return Some(i);
        }
        prev = c;
    }
    None
}
