use gozen_diagnostics::{Diagnostic, Fix, Severity, TextEdit};
use gozen_parser::{span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct UnnecessaryPass;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/unnecessaryPass",
    name: "unnecessaryPass",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: true,
    description: "`pass` is unnecessary when there are other statements in the same block.",
    explanation: "The `pass` statement is only needed as a placeholder in an otherwise empty body. If there are other statements in the same block, the `pass` is dead code and should be removed.",
};

impl Rule for UnnecessaryPass {
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

        walk_tree(root, source, |node, _src| {
            if node.kind() != "pass_statement" {
                return;
            }

            // Check if this pass is the only named statement in its parent body
            let mut cursor = node.walk();
            if !cursor.goto_parent() {
                return;
            }
            let parent = cursor.node();

            // Count named children that are actual statements (not comments)
            let mut statement_count = 0;
            for i in 0..parent.child_count() {
                if let Some(child) = parent.child(i) {
                    if child.is_named() && child.kind() != "comment" {
                        statement_count += 1;
                    }
                }
            }

            // If there's more than one statement, pass is unnecessary
            if statement_count > 1 {
                let span = span_from_node(node);
                // Build the removal span: include the full line with the pass
                let start_byte = node.start_byte();
                let mut end_byte = node.end_byte();
                // Extend to consume the trailing newline if present
                let bytes = source.as_bytes();
                if end_byte < bytes.len() && bytes[end_byte] == b'\n' {
                    end_byte += 1;
                } else if end_byte < bytes.len() && bytes[end_byte] == b'\r' {
                    end_byte += 1;
                    if end_byte < bytes.len() && bytes[end_byte] == b'\n' {
                        end_byte += 1;
                    }
                }
                // Also consume leading whitespace on the same line
                let mut remove_start = start_byte;
                while remove_start > 0 {
                    let prev = bytes[remove_start - 1];
                    if prev == b'\t' || prev == b' ' {
                        remove_start -= 1;
                    } else {
                        break;
                    }
                }
                // Only go back to line start if prev char is a newline
                if remove_start > 0
                    && (bytes[remove_start - 1] == b'\n' || bytes[remove_start - 1] == b'\r')
                {
                    // good, we're at the start of the line
                } else {
                    // Don't remove leading whitespace if there's other content before us
                    remove_start = start_byte;
                }

                let removal_span = gozen_diagnostics::Span {
                    start_byte: remove_start,
                    end_byte,
                    start_row: span.start_row,
                    start_col: span.start_col,
                    end_row: span.end_row,
                    end_col: span.end_col,
                };

                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Unnecessary `pass` statement.".to_string(),
                    file_path: None,
                    rule_id: None,
                    span,
                    notes: vec![],
                    fix: Some(Fix {
                        description: "Remove unnecessary pass".into(),
                        is_safe: true,
                        changes: vec![TextEdit {
                            span: removal_span,
                            new_text: String::new(),
                        }],
                    }),
                });
            }
        });

        diags
    }
}
