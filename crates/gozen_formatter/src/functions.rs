//! Formatting handlers for functions, constructors, classes, and lambdas.

use gozen_parser::{node_text, Node};

use crate::printer::{normalize_line_spacing, Printer};

impl Printer {
    /// Format a function or constructor definition.
    pub(crate) fn format_function_definition(&mut self, node: Node, source: &str) {
        let child_count = node.child_count();
        let mut i = 0;
        let mut found_name = false;
        // Emit "func <name>"
        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();
            if k == "identifier" || k == "function_name" || k == "name" {
                self.write("func ");
                self.write(node_text(child, source).trim());
                found_name = true;
                i += 1;
                break;
            }
            i += 1;
        }
        if !found_name {
            self.emit_raw(node, source);
            self.newline();
            if self.indent_level == 0 {
                self.newline();
            }
            return;
        }
        // Emit parameter list with potential wrapping
        let mut found_params = false;
        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();
            if k == "parameter_list" || k == "parameters" {
                self.format_param_list(child, source);
                found_params = true;
                i += 1;
                break;
            }
            i += 1;
        }
        if !found_params {
            self.write("()");
        }
        // Look for return type annotation (-> Type)
        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();
            if k == "type" || k == "return_type" {
                self.write(" -> ");
                self.write(node_text(child, source).trim());
                i += 1;
                break;
            }
            if k == "body"
                || k == "block"
                || k == "compound_statement"
                || k == "statement_list"
                || k == ":"
            {
                break;
            }
            if !child.is_named() {
                i += 1;
                continue;
            }
            i += 1;
        }
        self.write(":");
        self.newline();
        self.indent();
        let mut prev_end_row: Option<usize> = None;
        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            i += 1;
            if !child.is_named() {
                continue;
            }
            let body_kind = child.kind();
            if body_kind == "block"
                || body_kind == "compound_statement"
                || body_kind == "statement_list"
                || body_kind == "body"
            {
                for k in 0..child.child_count() {
                    if let Some(stmt) = child.child(k) {
                        if stmt.is_named() {
                            if self.try_append_inline_comment(stmt, prev_end_row, source) {
                                prev_end_row = Some(stmt.end_position().row);
                                continue;
                            }
                            if let Some(prev_end) = prev_end_row {
                                if stmt.start_position().row > prev_end + 1 {
                                    self.newline();
                                }
                            }
                            self.visit_node(stmt, source);
                            prev_end_row = Some(stmt.end_position().row);
                        }
                    }
                }
            } else if body_kind == "comment" || body_kind == "line_comment" {
                // Handle inline comments that follow the function signature
                if self.try_append_inline_comment(child, prev_end_row, source) {
                    prev_end_row = Some(child.end_position().row);
                    continue;
                }
                self.visit_node(child, source);
                prev_end_row = Some(child.end_position().row);
            } else if body_kind != "block_comment"
                && body_kind != ":"
                && body_kind != "type"
                && body_kind != "return_type"
            {
                if let Some(prev_end) = prev_end_row {
                    if child.start_position().row > prev_end + 1 {
                        self.newline();
                    }
                }
                self.visit_node(child, source);
                prev_end_row = Some(child.end_position().row);
            }
        }
        self.dedent();
        if self.indent_level == 0 {
            self.newline();
        }
    }

    /// Format an inner class definition: `class ClassName extends Base:`
    pub(crate) fn format_class_definition(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let child_count = node.child_count();
        let mut i = 0;
        let mut wrote_header = false;

        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();

            if !child.is_named() {
                let text = node_text(child, source).trim();
                if text == "class" {
                    self.write("class ");
                } else if text == "extends" {
                    self.write(" extends ");
                }
                i += 1;
                continue;
            }

            match k {
                "body" | "block" | "compound_statement" | "statement_list" | "class_body" => {
                    self.write(":");
                    self.newline();
                    wrote_header = true;
                    self.indent();
                    let mut prev_end_row: Option<usize> = None;
                    for j in 0..child.child_count() {
                        if let Some(stmt) = child.child(j) {
                            if stmt.is_named() {
                                if self.try_append_inline_comment(stmt, prev_end_row, source) {
                                    prev_end_row = Some(stmt.end_position().row);
                                    continue;
                                }
                                if let Some(prev_end) = prev_end_row {
                                    if stmt.start_position().row > prev_end + 1 {
                                        self.newline();
                                    }
                                }
                                self.visit_node(stmt, source);
                                prev_end_row = Some(stmt.end_position().row);
                            }
                        }
                    }
                    self.dedent();
                }
                _ => {
                    let text = node_text(child, source).trim();
                    self.write(text);
                }
            }
            i += 1;
        }
        if !wrote_header {
            self.newline();
        }
    }

    /// Format a lambda expression.
    pub(crate) fn format_lambda(&mut self, node: Node, source: &str) {
        let text = node_text(node, source);
        if text.contains('\n') {
            self.emit_raw(node, source);
        } else {
            self.write(&normalize_line_spacing(text.trim()));
        }
    }

    /// Format a function parameter list with smart wrapping.
    pub(crate) fn format_param_list(&mut self, node: Node, source: &str) {
        let text = node_text(node, source).trim().trim_end_matches(':');
        let total_width = self.current_line_len + text.len() + 1;
        if total_width <= self.line_width {
            let normalized = normalize_line_spacing(text);
            let normalized = self.normalize_trailing_comma_in_brackets(&normalized, '(', ')');
            self.write(&normalized);
            return;
        }
        let params = Self::named_children(node);
        if params.is_empty() {
            self.write("()");
            return;
        }
        self.write("(");
        self.newline();
        self.indent();
        for (idx, param) in params.iter().enumerate() {
            self.emit_indent();
            let param_text = node_text(*param, source).trim().to_string();
            self.write(&normalize_line_spacing(&param_text));
            if idx < params.len() - 1 || self.config.trailing_comma {
                self.write(",");
            }
            self.newline();
        }
        self.dedent();
        self.emit_indent();
        self.write(")");
    }
}
