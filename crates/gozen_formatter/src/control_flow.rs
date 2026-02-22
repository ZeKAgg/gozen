//! Formatting handlers for control flow: if, for, while, match statements.

use gozen_parser::{node_text, Node};

use crate::printer::{normalize_expression_spacing, normalize_line_spacing, Printer};

impl Printer {
    /// Format an if/elif/else statement with proper spacing normalization.
    pub(crate) fn format_if_statement(&mut self, node: Node, source: &str) {
        self.emit_indent();
        self.format_if_branch(node, source, "if");
    }

    fn write_multiline_inline_fragment(&mut self, text: &str) {
        let trimmed = text.trim_end_matches(['\n', '\r']);
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.is_empty() {
            return;
        }

        self.write(lines[0].trim());

        let mut base_indent = usize::MAX;
        for line in lines.iter().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            let leading = line
                .chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .count();
            base_indent = base_indent.min(leading);
        }
        if base_indent == usize::MAX {
            base_indent = 0;
        }

        for line in lines.iter().skip(1) {
            self.newline();
            if line.trim().is_empty() {
                continue;
            }
            let leading = line
                .chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .count();
            let extra_levels = leading.saturating_sub(base_indent);
            self.emit_indent();
            if extra_levels > 0 {
                self.write(&self.indent_str.repeat(extra_levels));
            }
            self.write(line.trim_start_matches([' ', '\t']));
        }
    }

    /// Format a single if/elif branch (recursive for elif chains).
    fn format_if_branch(&mut self, node: Node, source: &str, keyword: &str) {
        let child_count = node.child_count();
        let mut i = 0;
        let mut wrote_keyword = false;

        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();

            if !child.is_named() {
                let text = node_text(child, source).trim();
                if !wrote_keyword && (text == "if" || text == "elif") {
                    self.write(keyword);
                    self.write(" ");
                    wrote_keyword = true;
                }
                i += 1;
                continue;
            }

            match k {
                "body" | "block" | "compound_statement" | "statement_list" => {
                    self.write(":");
                    self.newline();
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
                "elif_clause" => {
                    self.emit_indent();
                    self.format_if_branch(child, source, "elif");
                }
                "else_clause" => {
                    self.emit_indent();
                    self.write("else:");
                    self.newline();
                    self.indent();
                    let mut prev_end_row: Option<usize> = None;
                    for j in 0..child.child_count() {
                        if let Some(inner) = child.child(j) {
                            if inner.is_named() {
                                let ik = inner.kind();
                                if ik == "body"
                                    || ik == "block"
                                    || ik == "compound_statement"
                                    || ik == "statement_list"
                                {
                                    for m in 0..inner.child_count() {
                                        if let Some(stmt) = inner.child(m) {
                                            if stmt.is_named() {
                                                if self.try_append_inline_comment(
                                                    stmt,
                                                    prev_end_row,
                                                    source,
                                                ) {
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
                                } else if self.try_append_inline_comment(
                                    inner,
                                    prev_end_row,
                                    source,
                                ) {
                                    prev_end_row = Some(inner.end_position().row);
                                } else {
                                    self.visit_node(inner, source);
                                    prev_end_row = Some(inner.end_position().row);
                                }
                            }
                        }
                    }
                    self.dedent();
                }
                _ => {
                    let text = node_text(child, source).trim();
                    if text.contains('\n') {
                        self.write_multiline_inline_fragment(text);
                    } else {
                        self.write(&normalize_expression_spacing(text));
                    }
                }
            }
            i += 1;
        }
    }

    /// Format a for statement: `for x in expr:`
    pub(crate) fn format_for_statement(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let child_count = node.child_count();
        let mut i = 0;
        let mut wrote_for = false;

        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();

            if !child.is_named() {
                let text = node_text(child, source).trim();
                if text == "for" && !wrote_for {
                    self.write("for ");
                    wrote_for = true;
                } else if text == "in" {
                    self.write(" in ");
                }
                i += 1;
                continue;
            }

            match k {
                "body" | "block" | "compound_statement" | "statement_list" => {
                    self.write(":");
                    self.newline();
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
                    self.write(&normalize_line_spacing(text));
                }
            }
            i += 1;
        }
    }

    /// Format a while statement: `while expr:`
    pub(crate) fn format_while_statement(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let child_count = node.child_count();
        let mut i = 0;
        let mut wrote_while = false;

        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();

            if !child.is_named() {
                let text = node_text(child, source).trim();
                if text == "while" && !wrote_while {
                    self.write("while ");
                    wrote_while = true;
                }
                i += 1;
                continue;
            }

            match k {
                "body" | "block" | "compound_statement" | "statement_list" => {
                    self.write(":");
                    self.newline();
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
                    self.write(&normalize_line_spacing(text));
                }
            }
            i += 1;
        }
    }

    /// Format a match statement: `match expr:` followed by match arms
    pub(crate) fn format_match_statement(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let child_count = node.child_count();
        let mut i = 0;
        let mut wrote_match = false;

        while i < child_count {
            let Some(child) = node.child(i) else {
                i += 1;
                continue;
            };
            let k = child.kind();

            if !child.is_named() {
                let text = node_text(child, source).trim();
                if text == "match" && !wrote_match {
                    self.write("match ");
                    wrote_match = true;
                }
                i += 1;
                continue;
            }

            match k {
                "match_body" | "body" | "block" | "compound_statement" | "statement_list" => {
                    self.write(":");
                    self.newline();
                    self.indent();
                    let mut prev_end_row: Option<usize> = None;
                    for j in 0..child.child_count() {
                        if let Some(arm) = child.child(j) {
                            if arm.is_named() {
                                if self.try_append_inline_comment(arm, prev_end_row, source) {
                                    prev_end_row = Some(arm.end_position().row);
                                    continue;
                                }
                                self.emit_indent();
                                self.emit_raw(arm, source);
                                self.newline();
                                prev_end_row = Some(arm.end_position().row);
                            }
                        }
                    }
                    self.dedent();
                }
                _ => {
                    let text = node_text(child, source).trim();
                    self.write(&normalize_line_spacing(text));
                }
            }
            i += 1;
        }
    }
}
