use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{call_name, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct PreferPreload;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/preferPreload",
    name: "preferPreload",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Use preload() instead of load() for constant resource paths.",
    explanation: "preload() loads resources at compile time which is faster and catches missing paths earlier. Use load() only for paths determined at runtime.",
};

impl Rule for PreferPreload {
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
            if name != "load" {
                return;
            }
            let text = node_text(node, src);

            // Check if the argument is a constant string (res:// path)
            // A constant string argument looks like: load("res://...")
            if text.contains("\"res://") || text.contains("'res://") {
                // Make sure the path isn't built dynamically (no concatenation)
                let arg_section = match text.find('(') {
                    Some(pos) => &text[pos..],
                    None => return, // Not a call expression — shouldn't happen but be defensive
                };
                if !arg_section.contains('+') && !arg_section.contains('%') {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "Use preload() instead of load() for constant resource paths."
                            .to_string(),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(node),
                        notes: vec![Note {
                            message:
                                "preload() loads at compile time, catching missing paths earlier."
                                    .to_string(),
                            span: None,
                        }],
                        fix: None,
                    });
                }
            }
        });
        diags
    }
}
