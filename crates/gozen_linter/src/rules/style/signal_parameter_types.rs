use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct SignalParameterTypes;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/signalParameterTypes",
    name: "signalParameterTypes",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Signal parameters should have type hints.",
    explanation: "Adding type hints to signal parameters improves type safety for signal handlers and documentation.",
};

impl Rule for SignalParameterTypes {
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
            let k = node.kind();
            if k != "signal_statement" && k != "signal_declaration" {
                return;
            }
            let text = node_text(node, src);

            // Only check signals that have parameters (contain parentheses)
            if !text.contains('(') {
                return;
            }

            // Extract the parameter section
            let param_start = match text.find('(') {
                Some(p) => p + 1,
                None => return,
            };
            let param_end = match text.rfind(')') {
                Some(p) => p,
                None => return,
            };
            if param_start >= param_end {
                return;
            }
            let params_text = &text[param_start..param_end];

            // Split on top-level commas only (respecting bracket depth for types like Array[int])
            for param in split_top_level_commas(params_text) {
                let param = param.trim();
                if param.is_empty() {
                    continue;
                }
                if !param.contains(':') {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Signal parameter \"{}\" should have a type hint.",
                            param.split_whitespace().next().unwrap_or(param)
                        ),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(node),
                        notes: vec![],
                        fix: None,
                    });
                }
            }
        });
        diags
    }
}

/// Split a string on commas, but only at the top level (not inside brackets or parens).
/// Handles types like `Array[Dictionary[String, int]]` correctly.
fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                result.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&s[start..]);
    result
}
