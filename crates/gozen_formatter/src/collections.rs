//! Formatting handlers for collections: arrays, dictionaries, and call arguments.

use gozen_parser::{node_text, Node};

use crate::printer::{normalize_line_spacing, Printer};

impl Printer {
    fn normalize_dictionary_brace_inner_spacing(&self, text: &str) -> String {
        let trimmed = text.trim();
        if trimmed == "{}" {
            return "{}".to_string();
        }
        if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
            return trimmed.to_string();
        }
        let inner = trimmed[1..trimmed.len() - 1].trim();
        if inner.is_empty() {
            "{}".to_string()
        } else {
            format!("{{ {} }}", inner)
        }
    }

    /// Format an array literal, wrapping to multi-line if it exceeds line_width.
    pub(crate) fn format_array(&mut self, node: Node, source: &str) {
        if self.fits_on_line(node, source) {
            let text = node_text(node, source).trim().to_string();
            let normalized = normalize_line_spacing(&text);
            let normalized = self.normalize_trailing_comma_in_brackets(&normalized, '[', ']');
            self.write(&normalized);
            return;
        }
        let items = Self::named_children(node);
        if items.is_empty() {
            self.write("[]");
            return;
        }
        self.emit_multiline_list(&items, source, "[", "]");
    }

    /// Format a dictionary literal, wrapping to multi-line if it exceeds line_width.
    pub(crate) fn format_dictionary(&mut self, node: Node, source: &str) {
        if self.fits_on_line(node, source) {
            let text = node_text(node, source).trim().to_string();
            let normalized = normalize_line_spacing(&text);
            let normalized = self.normalize_trailing_comma_in_brackets(&normalized, '{', '}');
            let normalized = self.normalize_dictionary_brace_inner_spacing(&normalized);
            self.write(&normalized);
            return;
        }
        let items = Self::named_children(node);
        if items.is_empty() {
            self.write("{}");
            return;
        }
        self.emit_multiline_list(&items, source, "{", "}");
    }

    /// Format function call arguments, wrapping to multi-line if needed.
    pub(crate) fn format_call_args(&mut self, node: Node, source: &str) {
        if self.fits_on_line(node, source) {
            let text = node_text(node, source).trim().to_string();
            let normalized = normalize_line_spacing(&text);
            let normalized = self.normalize_trailing_comma_in_brackets(&normalized, '(', ')');
            self.write(&normalized);
            return;
        }
        let items = Self::named_children(node);
        if items.is_empty() {
            self.write("()");
            return;
        }
        self.emit_multiline_list(&items, source, "(", ")");
    }
}
