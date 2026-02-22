use std::collections::HashMap;

use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{call_name, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct DuplicatedLoad;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/duplicatedLoad",
    name: "duplicatedLoad",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Same `load()` or `preload()` path used more than once.",
    explanation: "Calling `load()` or `preload()` with the same path in multiple places is wasteful and error-prone. Extract the resource path to a constant: `const MyScene = preload(\"res://scene.tscn\")`.",
};

impl Rule for DuplicatedLoad {
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
        // Collect all load/preload calls with their path arguments
        // Map: path_string -> list of spans
        let mut loads: HashMap<String, Vec<gozen_diagnostics::Span>> = HashMap::new();

        walk_tree(root, source, |node, src| {
            let kind = node.kind();
            if kind != "call" && kind != "call_expression" {
                return;
            }
            let name = call_name(node, src);
            if name != "load" && name != "preload" {
                return;
            }
            // Extract the first string argument
            if let Some(path) = extract_first_string_arg(node, src) {
                loads.entry(path).or_default().push(span_from_node(node));
            }
        });

        let mut diags = Vec::new();
        for (path, spans) in &loads {
            if spans.len() < 2 {
                continue;
            }
            // Report on the second and subsequent occurrences
            for span in &spans[1..] {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Duplicated load path \"{}\". Extract to a constant.", path),
                    file_path: None,
                    rule_id: None,
                    span: *span,
                    notes: vec![Note {
                        message: format!("First loaded at line {}.", spans[0].start_row + 1),
                        span: Some(spans[0]),
                    }],
                    fix: None,
                });
            }
        }
        diags
    }
}

/// Extract the first string literal argument from a call node.
fn extract_first_string_arg<'a>(
    call_node: gozen_parser::Node<'a>,
    source: &'a str,
) -> Option<String> {
    // Look for an arguments node or directly find a string child
    for i in 0..call_node.child_count() {
        if let Some(child) = call_node.child(i) {
            let ck = child.kind();
            if ck == "arguments" || ck == "argument_list" {
                // Look inside arguments for a string
                for j in 0..child.child_count() {
                    if let Some(arg) = child.child(j) {
                        if arg.kind() == "string" {
                            let text = node_text(arg, source);
                            // Strip quotes
                            let stripped = text
                                .trim_start_matches('"')
                                .trim_end_matches('"')
                                .trim_start_matches('\'')
                                .trim_end_matches('\'');
                            return Some(stripped.to_string());
                        }
                    }
                }
            } else if ck == "string" {
                let text = node_text(child, source);
                let stripped = text
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .trim_start_matches('\'')
                    .trim_end_matches('\'');
                return Some(stripped.to_string());
            }
        }
    }
    None
}
