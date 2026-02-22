//! Formatting handlers for declarations: extends, class_name, signal, const, enum,
//! variable statements, source file, comments, annotations, and regions.

use gozen_parser::{node_text, Node};

use crate::printer::{
    normalize_expression_spacing, normalize_inline_dictionary_braces, normalize_line_spacing,
    normalize_variable_spacing, Printer,
};

impl Printer {
    /// Visit a top-level source file node, inserting blank lines between declarations.
    /// Preserves blank lines from the original source: if there was one blank line
    /// between two declarations, one blank line is emitted; if there were two or more,
    /// two blank lines are emitted (capped at 2 to avoid excessive whitespace).
    pub(crate) fn visit_source_file(&mut self, node: Node, source: &str) {
        let mut prev_end_row: Option<usize> = None;
        let mut prev_is_func = false;
        let mut i = 0;
        while i < node.child_count() {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            if child.is_named() {
                // Inline comment: append to previous line
                if self.try_append_inline_comment(child, prev_end_row, source) {
                    prev_end_row = Some(child.end_position().row);
                    i += 1;
                    continue;
                }
                if let Some(prev_end) = prev_end_row {
                    let cur_start = child.start_position().row;
                    let gap = cur_start.saturating_sub(prev_end);
                    let cur_is_func = node_text(child, source).trim_start().starts_with("func ");
                    if prev_is_func && cur_is_func {
                        if gap > 2 {
                            self.newline();
                        }
                    } else if gap > 2 {
                        // Two or more blank lines in source -> emit 2 blank lines
                        self.newline();
                        self.newline();
                    } else if gap > 1 {
                        // One blank line in source -> emit 1 blank line
                        self.newline();
                    }
                }
                let cur_is_func = node_text(child, source).trim_start().starts_with("func ");
                self.visit_node(child, source);
                prev_end_row = Some(child.end_position().row);
                prev_is_func = cur_is_func;
            }
            i += 1;
        }
    }

    /// Emit a simple one-line statement (extends, class_name, signal, etc.).
    /// Normalizes spacing: collapses multiple spaces, strips spaces inside brackets.
    pub(crate) fn emit_simple_statement(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let text = node_text(node, source).trim();
        if node.kind() == "expression_statement" {
            let normalized = normalize_expression_spacing(text);
            self.write(&normalize_line_spacing(&normalized));
        } else {
            self.write(&normalize_line_spacing(text));
        }
        self.newline();
    }

    /// Format a variable declaration, normalizing spacing for single-line vars
    /// and preserving structure for multi-line vars (setget).
    pub(crate) fn format_variable_statement(&mut self, node: Node, source: &str) {
        let text = node_text(node, source);
        if text.contains('\n') {
            self.emit_raw(node, source);
            return;
        }
        let normalized = normalize_variable_spacing(text.trim());
        let normalized = normalize_inline_dictionary_braces(&normalized);
        self.write(&normalized);
    }

    /// Emit a comment: preserve original text, re-indent, trailing newline.
    pub(crate) fn format_comment(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let text = node_text(node, source).trim();
        self.write(text);
        self.newline();
    }
}
