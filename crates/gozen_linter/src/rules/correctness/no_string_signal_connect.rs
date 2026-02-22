use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{call_name, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoStringSignalConnect;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noStringSignalConnect",
    name: "noStringSignalConnect",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Old Godot 3 string-based signal connection syntax.",
    explanation: "Godot 4 uses callable syntax for signal connections: `signal.connect(callable)`. The old `connect(\"signal\", target, \"method\")` pattern is deprecated and not type-safe.",
};

impl Rule for NoStringSignalConnect {
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

        walk_tree(root, source, |node, src| {
            if node.kind() != "call_expression" && node.kind() != "call" {
                return;
            }
            let name = call_name(node, src);
            if name != "connect" {
                return;
            }
            let text = node_text(node, src);

            // Godot 3 pattern: connect("signal_name", target, "method_name")
            // Has 3+ arguments where the first is a string literal.
            // Godot 4 pattern: signal.connect(callable) — the `connect` is called on a signal object.
            // Heuristic: if the first argument is a string literal AND there are 3+ args,
            // this is the old Godot 3 pattern.

            let arg_count = count_arguments(text);
            if arg_count >= 3 && first_arg_is_string(text) {
                // Likely old-style: connect("signal", target, "method")
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Use Godot 4 signal syntax: `signal_name.connect(callable)` instead of `connect(\"signal\", target, \"method\")`.".to_string(),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(node),
                    notes: vec![Note {
                        message: "Example: button.pressed.connect(_on_button_pressed)".to_string(),
                        span: None,
                    }],
                    fix: None,
                });
            }
        });
        diags
    }
}

/// Check if the first argument to the call is a string literal (starts with `"` or `'`).
fn first_arg_is_string(text: &str) -> bool {
    if let Some(paren_pos) = text.find('(') {
        let after_paren = &text[paren_pos + 1..];
        let trimmed = after_paren.trim_start();
        trimmed.starts_with('"') || trimmed.starts_with('\'')
    } else {
        false
    }
}

/// Count the number of top-level arguments by counting commas outside strings and parens.
fn count_arguments(text: &str) -> usize {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut string_char = '"';
    let mut commas = 0;
    let mut prev = '\0';
    let mut started = false;

    for c in text.chars() {
        if in_string {
            if c == string_char && prev != '\\' {
                in_string = false;
            }
        } else {
            match c {
                '"' | '\'' => {
                    in_string = true;
                    string_char = c;
                }
                '(' => {
                    depth += 1;
                    if !started {
                        started = true;
                    }
                }
                ')' => depth -= 1,
                ',' if depth == 1 => commas += 1,
                _ => {}
            }
        }
        prev = c;
    }
    if started {
        commas + 1
    } else {
        0
    }
}
