use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::Tree;

use crate::rule::{Rule, RuleMetadata};

pub struct LineLength;

/// Default maximum line length per the GDScript style guide.
const MAX_LINE_LENGTH: usize = 100;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/lineLength",
    name: "lineLength",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Lines exceeding 100 characters.",
    explanation: "The GDScript style guide recommends keeping lines under 100 characters (prefer 80) for readability across displays and side-by-side editing.",
};

impl Rule for LineLength {
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
            if line_bytes > MAX_LINE_LENGTH {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "Line exceeds {} characters ({} chars). Note: measured in bytes, not display width.",
                        MAX_LINE_LENGTH, line_bytes
                    ),
                    file_path: None,
                    rule_id: None,
                    span: Span {
                        start_byte: byte_offset + MAX_LINE_LENGTH,
                        end_byte: byte_offset + line_bytes,
                        start_row: line_idx,
                        start_col: MAX_LINE_LENGTH,
                        end_row: line_idx,
                        end_col: line_bytes,
                    },
                    notes: vec![],
                    fix: None,
                });
            }
            // Advance past line content and actual line ending (\r\n or \n)
            byte_offset += line_bytes;
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
