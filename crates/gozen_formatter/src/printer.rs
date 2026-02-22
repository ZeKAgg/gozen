use gozen_config::FormatterConfig;
use gozen_parser::{node_text, Node, Tree};

/// Remove spaces inside (), [], {} and collapse multiple spaces.
/// Uses &str slicing to preserve multi-byte UTF-8 characters correctly.
pub(crate) fn normalize_line_spacing(line: &str) -> String {
    let mut out = String::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_string = false;
    let mut in_triple_string = false;
    let mut quote = b'"';
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut depth_brace = 0i32;
    let mut last_was_space = false;
    while i < len {
        let c = bytes[i];
        if in_triple_string {
            let start = i;
            if c == b'\\' && i + 1 < len {
                i += 2;
            } else if c == quote && i + 2 < len && bytes[i + 1] == quote && bytes[i + 2] == quote {
                // End of triple-quoted string
                i += 3;
                in_triple_string = false;
            } else {
                i += 1;
            }
            out.push_str(&line[start..i]);
            last_was_space = false;
            continue;
        }
        if in_string {
            let start = i;
            if c == b'\\' && i + 1 < len {
                i += 2; // skip escaped char (escape + the next byte)
            } else {
                if c == quote {
                    in_string = false;
                }
                i += 1;
            }
            out.push_str(&line[start..i]);
            last_was_space = false;
            continue;
        }
        // Detect triple-quote opening before single-quote
        if (c == b'"' || c == b'\'') && i + 2 < len && bytes[i + 1] == c && bytes[i + 2] == c {
            in_triple_string = true;
            quote = c;
            out.push_str(&line[i..i + 3]);
            last_was_space = false;
            i += 3;
            continue;
        }
        if c == b'"' || c == b'\'' {
            in_string = true;
            quote = c;
            out.push_str(&line[i..i + 1]);
            last_was_space = false;
            i += 1;
            continue;
        }
        match c {
            b'(' => {
                if out.ends_with(' ') {
                    let trimmed = out.trim_end_matches(' ');
                    let prev_non_ws = trimmed.as_bytes().last().copied();
                    let is_call_capable = matches!(
                        prev_non_ws,
                        Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b')' | b']')
                    );
                    if is_call_capable {
                        let mut j = trimmed.len();
                        while j > 0 {
                            let b = trimmed.as_bytes()[j - 1];
                            if b.is_ascii_alphanumeric() || b == b'_' {
                                j -= 1;
                            } else {
                                break;
                            }
                        }
                        let tail = &trimmed[j..];
                        if tail != "if" && tail != "for" && tail != "while" && tail != "match" {
                            out.truncate(trimmed.len());
                        }
                    }
                }
                depth_paren += 1;
                out.push('(');
                last_was_space = false;
                i += 1;
                continue;
            }
            b')' => {
                depth_paren -= 1;
                out.push(')');
                last_was_space = false;
                i += 1;
                continue;
            }
            b'[' => {
                depth_bracket += 1;
                out.push('[');
                last_was_space = false;
                i += 1;
                continue;
            }
            b']' => {
                depth_bracket -= 1;
                out.push(']');
                last_was_space = false;
                i += 1;
                continue;
            }
            b'{' => {
                depth_brace += 1;
                out.push('{');
                last_was_space = false;
                i += 1;
                continue;
            }
            b'}' => {
                depth_brace -= 1;
                out.push('}');
                last_was_space = false;
                i += 1;
                continue;
            }
            b',' => {
                out.push(',');
                // Always add a space after commas inside brackets/parens/braces
                if depth_paren > 0 || depth_bracket > 0 || depth_brace > 0 {
                    out.push(' ');
                    last_was_space = true;
                } else {
                    last_was_space = false;
                }
                i += 1;
                continue;
            }
            b':' => {
                out.push(':');
                // Always add a space after colons inside parens (type annotations: value: float)
                if depth_paren > 0 {
                    out.push(' ');
                    last_was_space = true;
                } else {
                    last_was_space = false;
                }
                i += 1;
                continue;
            }
            b' ' | b'\t' | b'\r' | b'\n' => {
                if depth_paren > 0 || depth_bracket > 0 || depth_brace > 0 {
                    i += 1;
                    continue;
                }
                if !last_was_space {
                    out.push(' ');
                }
                last_was_space = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        // Non-special character: find the span of regular content and copy it
        let start = i;
        i += 1;
        while i < len && !in_string {
            let b = bytes[i];
            if matches!(
                b,
                b'"' | b'\''
                    | b','
                    | b':'
                    | b'('
                    | b')'
                    | b'['
                    | b']'
                    | b'{'
                    | b'}'
                    | b' '
                    | b'\t'
                    | b'\\'
                    | b'\r'
                    | b'\n'
            ) {
                break;
            }
            i += 1;
        }
        out.push_str(&line[start..i]);
        last_was_space = false;
    }
    out
}

/// Normalize spacing in one-line expressions while preserving strings/comments.
pub(crate) fn normalize_expression_spacing(text: &str) -> String {
    let mut out = String::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_string = false;
    let mut in_triple_string = false;
    let mut quote = b'"';

    while i < len {
        let c = bytes[i];

        if in_triple_string {
            let start = i;
            if c == b'\\' && i + 1 < len {
                i += 2;
            } else if c == quote && i + 2 < len && bytes[i + 1] == quote && bytes[i + 2] == quote {
                i += 3;
                in_triple_string = false;
            } else {
                i += 1;
            }
            out.push_str(&text[start..i]);
            continue;
        }

        if in_string {
            let start = i;
            if c == b'\\' && i + 1 < len {
                i += 2;
            } else {
                if c == quote {
                    in_string = false;
                }
                i += 1;
            }
            out.push_str(&text[start..i]);
            continue;
        }

        if (c == b'"' || c == b'\'') && i + 2 < len && bytes[i + 1] == c && bytes[i + 2] == c {
            in_triple_string = true;
            quote = c;
            out.push_str(&text[i..i + 3]);
            i += 3;
            continue;
        }
        if c == b'"' || c == b'\'' {
            in_string = true;
            quote = c;
            out.push_str(&text[i..i + 1]);
            i += 1;
            continue;
        }

        if c == b'#' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&text[i..]);
            break;
        }

        if c == b'(' && out.ends_with(' ') {
            let trimmed = out.trim_end_matches(' ');
            let prev_non_ws = trimmed.as_bytes().last().copied();
            let is_call_capable = matches!(
                prev_non_ws,
                Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b')' | b']')
            );
            if is_call_capable {
                let mut j = trimmed.len();
                while j > 0 {
                    let b = trimmed.as_bytes()[j - 1];
                    if b.is_ascii_alphanumeric() || b == b'_' {
                        j -= 1;
                    } else {
                        break;
                    }
                }
                let tail = &trimmed[j..];
                if tail != "if" && tail != "for" && tail != "while" && tail != "match" {
                    out.truncate(trimmed.len());
                }
            }
        }

        let two = if i + 1 < len {
            Some(&text[i..i + 2])
        } else {
            None
        };
        if let Some(op) = two {
            if matches!(
                op,
                "==" | "!=" | "<=" | ">=" | "+=" | "-=" | "*=" | "/=" | "%="
            ) {
                while out.ends_with(' ') {
                    out.pop();
                }
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(op);
                out.push(' ');
                i += 2;
                while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                continue;
            }
        }

        if c == b'=' {
            while out.ends_with(' ') {
                out.pop();
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push('=');
            out.push(' ');
            i += 1;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            continue;
        }

        if c == b'<' || c == b'>' {
            while out.ends_with(' ') {
                out.pop();
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push(c as char);
            out.push(' ');
            i += 1;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            continue;
        }

        if c == b'+' || c == b'-' || c == b'*' || c == b'/' || c == b'%' {
            let prev_non_ws = out
                .as_bytes()
                .iter()
                .rev()
                .find(|b| **b != b' ' && **b != b'\t')
                .copied();
            let unary_context = prev_non_ws.is_none()
                || matches!(
                    prev_non_ws,
                    Some(b'(' | b'[' | b'{' | b',' | b'=' | b':' | b'!' | b'<' | b'>')
                );

            if unary_context {
                out.push(c as char);
                i += 1;
                while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                continue;
            }

            while out.ends_with(' ') {
                out.pop();
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push(c as char);
            out.push(' ');
            i += 1;
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            continue;
        }

        if c == b' ' || c == b'\t' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            i += 1;
            continue;
        }

        out.push(c as char);
        i += 1;
    }

    out.trim().to_string()
}

pub(crate) fn normalize_inline_dictionary_braces(text: &str) -> String {
    let trimmed = text.trim();
    if !(trimmed.contains('{') && trimmed.contains('}')) {
        return trimmed.to_string();
    }
    if let (Some(open), Some(close)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if close > open {
            let inner = trimmed[open + 1..close].trim();
            if inner.is_empty() {
                return format!("{}{}{}", &trimmed[..open], "{}", &trimmed[close + 1..]);
            }
            let mut out = String::new();
            out.push_str(&trimmed[..open + 1]);
            out.push(' ');
            out.push_str(inner);
            out.push(' ');
            out.push('}');
            out.push_str(&trimmed[close + 1..]);
            return out;
        }
    }
    trimmed.to_string()
}

/// Normalize spacing in variable declaration: "var x:int=5" -> "var x: int = 5"
/// Uses &str slicing to preserve multi-byte UTF-8 characters correctly.
pub(crate) fn normalize_variable_spacing(text: &str) -> String {
    let mut out = String::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_string = false;
    let mut in_triple_string = false;
    let mut quote = b'"';
    let mut last_was_space = false;
    while i < len {
        let c = bytes[i];
        if in_triple_string {
            let start = i;
            if c == b'\\' && i + 1 < len {
                i += 2;
            } else if c == quote && i + 2 < len && bytes[i + 1] == quote && bytes[i + 2] == quote {
                i += 3;
                in_triple_string = false;
            } else {
                i += 1;
            }
            out.push_str(&text[start..i]);
            last_was_space = false;
            continue;
        }
        if in_string {
            let start = i;
            if c == b'\\' && i + 1 < len {
                i += 2;
            } else {
                if c == quote {
                    in_string = false;
                }
                i += 1;
            }
            out.push_str(&text[start..i]);
            last_was_space = false;
            continue;
        }
        // Detect triple-quote opening before single-quote
        if (c == b'"' || c == b'\'') && i + 2 < len && bytes[i + 1] == c && bytes[i + 2] == c {
            in_triple_string = true;
            quote = c;
            out.push_str(&text[i..i + 3]);
            last_was_space = false;
            i += 3;
            continue;
        }
        if c == b'"' || c == b'\'' {
            in_string = true;
            quote = c;
            out.push_str(&text[i..i + 1]);
            last_was_space = false;
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
            i += 1;
            continue;
        }
        if c == b':' {
            if out.trim_end().ends_with(':') {
                i += 1;
                continue;
            }
            // Strip trailing whitespace before colon (e.g. "var x : int" → "var x: int")
            let trimmed = out.trim_end();
            out.truncate(trimmed.len());

            // Peek ahead: if next non-whitespace is '=', handle ':=' as one token
            let mut peek = i + 1;
            while peek < len && (bytes[peek] == b' ' || bytes[peek] == b'\t') {
                peek += 1;
            }
            if peek < len && bytes[peek] == b'=' {
                // Inferred type operator :=
                out.push_str(" := ");
                last_was_space = true;
                i = peek + 1; // skip past the '='
                continue;
            }

            // Regular type hint colon (e.g. "var x: int = 5")
            out.push(':');
            out.push(' ');
            last_was_space = true;
            i += 1;
            continue;
        }
        if c == b'=' {
            // Peek ahead for == (equality comparison)
            if i + 1 < len && bytes[i + 1] == b'=' {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str("==");
                out.push(' ');
                last_was_space = true;
                i += 2;
                continue;
            }
            // Check if this = is part of a compound operator (!=, >=, <=, +=, -=, *=, /=, %=)
            // The prefix character is already in `out`; don't insert a space between it and =.
            let prev = {
                let t = out.trim_end();
                t.as_bytes().last().copied()
            };
            if matches!(
                prev,
                Some(b'!' | b'>' | b'<' | b'+' | b'-' | b'*' | b'/' | b'%')
            ) {
                let trimmed_len = out.trim_end().len();
                out.truncate(trimmed_len);
                // Ensure there is a space before the operator prefix (e.g. "count!" → "count !")
                if trimmed_len >= 2 {
                    let before_prefix = out.as_bytes()[trimmed_len - 2];
                    if before_prefix != b' ' && before_prefix != b'\t' {
                        let prefix_char = out.pop().unwrap();
                        out.push(' ');
                        out.push(prefix_char);
                    }
                }
                out.push_str("= ");
                last_was_space = true;
                i += 1;
                continue;
            }
            // Regular assignment =
            if !out.ends_with(' ') {
                out.push(' ');
            }
            out.push('=');
            out.push(' ');
            last_was_space = true;
            i += 1;
            continue;
        }
        // Comma: always add a space after for consistent formatting
        if c == b',' {
            out.push(',');
            out.push(' ');
            last_was_space = true;
            i += 1;
            continue;
        }
        // Non-special character: copy a span of regular content
        let start = i;
        i += 1;
        while i < len && !in_string {
            let b = bytes[i];
            if matches!(
                b,
                b'"' | b'\'' | b',' | b' ' | b'\t' | b':' | b'=' | b'\\' | b'\r' | b'\n'
            ) {
                break;
            }
            i += 1;
        }
        out.push_str(&text[start..i]);
        last_was_space = false;
    }
    out.trim().to_string()
}

// ── Printer struct and core methods ─────────────────────────────────────

pub struct Printer {
    pub(crate) output: String,
    pub(crate) indent_level: usize,
    pub(crate) indent_str: String,
    pub(crate) line_width: usize,
    pub(crate) current_line_len: usize,
    pub(crate) config: FormatterConfig,
    /// Last thing we wrote was newline (so we need to write indent before next content)
    pub(crate) need_indent: bool,
}

impl Printer {
    pub fn new(config: &FormatterConfig) -> Self {
        let indent_str = match config.indent_style.as_str() {
            "space" => " ".repeat(config.indent_width),
            _ => "\t".to_string(),
        };
        Self {
            output: String::new(),
            indent_level: 0,
            indent_str,
            line_width: config.line_width,
            current_line_len: 0,
            config: config.clone(),
            need_indent: true,
        }
    }

    pub(crate) fn write(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                self.output.push('\n');
                self.current_line_len = 0;
                self.need_indent = true;
            } else {
                if self.need_indent && ch != ' ' && ch != '\t' {
                    let indent = self.indent_str.repeat(self.indent_level);
                    self.output.push_str(&indent);
                    self.current_line_len = indent.len();
                    self.need_indent = false;
                }
                if !self.need_indent {
                    self.output.push(ch);
                    if ch != ' ' && ch != '\t' {
                        self.current_line_len += 1;
                    }
                }
            }
        }
    }

    pub(crate) fn newline(&mut self) {
        self.write("\n");
    }

    /// Try to append an inline comment to the current line instead of emitting
    /// it on its own line. Returns `true` if the comment was on the same source
    /// line as `prev_end_row` and was appended inline.
    pub(crate) fn try_append_inline_comment(
        &mut self,
        child: Node,
        prev_end_row: Option<usize>,
        source: &str,
    ) -> bool {
        let kind = child.kind();
        if kind != "comment" && kind != "line_comment" {
            return false;
        }
        if let Some(prev_row) = prev_end_row {
            if child.start_position().row == prev_row {
                // This comment is on the same line as the previous statement.
                // Strip the trailing newline we already wrote so we can append inline.
                if self.output.ends_with('\n') {
                    self.output.pop();
                    self.need_indent = false;
                }
                let text = node_text(child, source).trim();
                self.write("  ");
                self.write(text);
                self.newline();
                return true;
            }
        }
        false
    }

    pub(crate) fn indent(&mut self) {
        self.indent_level += 1;
    }

    pub(crate) fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(1);
    }

    pub(crate) fn emit_indent(&mut self) {
        if self.need_indent {
            let indent = self.indent_str.repeat(self.indent_level);
            self.output.push_str(&indent);
            self.current_line_len = indent.len();
            self.need_indent = false;
        }
    }

    /// Calculate the visual width of the current indent level.
    pub(crate) fn indent_width(&self) -> usize {
        self.indent_level * self.indent_str.len()
    }

    /// Measure the flat (single-line) width of a node's text content.
    pub(crate) fn flat_width(&self, node: Node, source: &str) -> usize {
        let text = node_text(node, source);
        let mut len = 0usize;
        let mut first = true;
        for word in text.split_whitespace() {
            if !first {
                len += 1;
            }
            len += word.len();
            first = false;
        }
        len
    }

    /// Check whether a node's flat representation fits within the remaining line budget.
    pub(crate) fn fits_on_line(&self, node: Node, source: &str) -> bool {
        let avail = self.line_width.saturating_sub(self.indent_width());
        self.flat_width(node, source) <= avail
    }

    /// Collect the named children of a node into a Vec.
    pub(crate) fn named_children(node: Node) -> Vec<Node> {
        let mut out = Vec::new();
        for i in 0..node.child_count() {
            if let Some(c) = node.child(i) {
                if c.is_named() {
                    out.push(c);
                }
            }
        }
        out
    }

    /// Emit a list of items in multi-line format (one per line), with optional trailing comma.
    pub(crate) fn emit_multiline_list(
        &mut self,
        items: &[Node],
        source: &str,
        open: &str,
        close: &str,
    ) {
        self.write(open);
        self.newline();
        self.indent();
        for (idx, item) in items.iter().enumerate() {
            self.emit_indent();
            let text = node_text(*item, source).trim().to_string();
            self.write(&normalize_line_spacing(&text));
            if idx < items.len() - 1 || self.config.trailing_comma {
                self.write(",");
            }
            self.newline();
        }
        self.dedent();
        self.emit_indent();
        self.write(close);
    }

    /// Normalize trailing comma inside single-line bracketed expressions.
    pub(crate) fn normalize_trailing_comma_in_brackets(
        &self,
        text: &str,
        _open: char,
        close: char,
    ) -> String {
        if let Some(close_pos) = text.rfind(close) {
            let before_close = text[..close_pos].trim_end();
            let after_close = &text[close_pos..];
            if before_close.ends_with(',') {
                let trimmed = before_close.trim_end_matches(',').trim_end();
                return format!("{}{}", trimmed, after_close);
            }
        }
        text.to_string()
    }

    /// Emit raw source for a node, re-indenting to the current indent level.
    pub(crate) fn emit_raw(&mut self, node: Node, source: &str) {
        let text = node_text(node, source);
        let trimmed = text.trim_start_matches([' ', '\t', '\n', '\r']);
        let lines: Vec<&str> = trimmed.lines().collect();

        let base_indent = node.start_position().column;

        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            let stripped = if i > 0 {
                let leading = line.len() - line.trim_start_matches([' ', '\t']).len();
                let strip = leading.min(base_indent);
                &line[strip..]
            } else {
                line
            };
            let stripped = stripped.trim_end();
            if !stripped.is_empty() {
                self.emit_indent();
                self.write(stripped);
            }
        }
    }

    pub fn print(&mut self, tree: &Tree, source: &str) -> String {
        if source.trim().is_empty() {
            return source.to_string();
        }
        let root = tree.root_node();
        self.visit_node(root, source);
        let trimmed = self.output.trim_end_matches('\n');
        self.output = trimmed.to_string();
        self.output.push('\n');
        self.output.clone()
    }

    // ── Central dispatch ────────────────────────────────────────────────

    pub(crate) fn visit_node(&mut self, node: Node, source: &str) {
        let kind = node.kind();

        // Error recovery: preserve raw source text for parse-error regions
        if kind == "ERROR" || kind == "MISSING" {
            self.emit_error_node(node, source);
            return;
        }

        match kind {
            // Source file (root)
            "source_file" | "source" | "program" | "module" | "file" => {
                self.visit_source_file(node, source);
            }

            // Comments
            "comment" => {
                self.format_comment(node, source);
            }

            // Declarations
            "extends_statement"
            | "class_name_statement"
            | "signal_statement"
            | "signal_declaration"
            | "enum_definition"
            | "enum_statement"
            | "tool_statement"
            | "region_start"
            | "region_end"
            | "annotation" => {
                self.emit_simple_statement(node, source);
            }

            // Const declarations (use variable spacing for := and : Type = patterns)
            "const_statement" | "constant_definition" => {
                self.emit_indent();
                self.format_variable_statement(node, source);
                self.newline();
            }

            // Variable declarations
            "variable_statement"
            | "variable_declaration"
            | "local_variable_statement"
            | "export_variable_statement"
            | "onready_variable_statement" => {
                self.emit_indent();
                self.format_variable_statement(node, source);
                self.newline();
            }

            // Functions and classes
            "function_definition" | "constructor_definition" => {
                self.emit_indent();
                self.format_function_definition(node, source);
            }
            "class_definition" => {
                self.format_class_definition(node, source);
            }
            "lambda" => {
                self.format_lambda(node, source);
            }

            // Control flow
            "if_statement" => self.format_if_statement(node, source),
            "for_statement" => self.format_for_statement(node, source),
            "while_statement" => self.format_while_statement(node, source),
            "match_statement" => self.format_match_statement(node, source),

            // Collections
            "array" | "array_literal" => self.format_array(node, source),
            "dictionary" | "dictionary_literal" => self.format_dictionary(node, source),
            "argument_list" | "arguments" => self.format_call_args(node, source),

            // Statements
            "expression_statement"
            | "return_statement"
            | "pass_statement"
            | "break_statement"
            | "continue_statement"
            | "breakpoint_statement" => {
                self.emit_simple_statement(node, source);
            }

            // Block / body containers
            "block" | "compound_statement" | "body" => {
                self.visit_block(node, source);
            }

            _ => {
                self.visit_fallback(node, source);
            }
        }
    }

    /// Emit preserved text for parse-error regions.
    fn emit_error_node(&mut self, node: Node, source: &str) {
        let text = node_text(node, source);
        let trimmed = text.trim_end();
        if !trimmed.is_empty() {
            self.emit_indent();
            self.write(trimmed);
            self.newline();
        }
    }

    /// Visit a block/compound_statement, indenting its children.
    /// Preserves blank lines from the original source and keeps inline comments
    /// on the same line as the preceding statement.
    pub(crate) fn visit_block(&mut self, node: Node, source: &str) {
        self.indent();
        let mut prev_end_row: Option<usize> = None;
        for i in 0..node.child_count() {
            let Some(child) = node.child(i) else {
                continue;
            };
            if child.is_named() {
                // Inline comment: append to previous line instead of new line
                if self.try_append_inline_comment(child, prev_end_row, source) {
                    prev_end_row = Some(child.end_position().row);
                    continue;
                }
                // Preserve blank lines from original source
                if let Some(prev_end) = prev_end_row {
                    let cur_start = child.start_position().row;
                    if cur_start > prev_end + 1 {
                        self.newline();
                    }
                }
                self.visit_node(child, source);
                prev_end_row = Some(child.end_position().row);
            }
        }
        self.dedent();
    }

    /// Fallback for unknown node types: recurse into named children or emit raw.
    fn visit_fallback(&mut self, node: Node, source: &str) {
        if node.child_count() > 0 && node.child(0).map(|c| c.is_named()).unwrap_or(false) {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.is_named() {
                        self.visit_node(child, source);
                    }
                }
            }
        } else {
            self.emit_indent();
            let text = node_text(node, source);
            let on_one_line = !text.contains('\n');
            if on_one_line {
                self.write(&normalize_line_spacing(text.trim()));
            } else {
                self.emit_raw(node, source);
            }
        }
    }
}
