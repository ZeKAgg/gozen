use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoOnreadyWithExport;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noOnreadyWithExport",
    name: "noOnreadyWithExport",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "@onready combined with @export on the same variable.",
    explanation: "Applying @onready and @export to the same variable causes the @onready default to override the exported value after _ready(). Godot treats this as an error (ONREADY_WITH_EXPORT).",
};

impl Rule for NoOnreadyWithExport {
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
        // Track flagged byte positions to avoid duplicate diagnostics
        let mut flagged_positions = std::collections::HashSet::new();
        walk_tree(root, source, |node, src| {
            let k = node.kind();
            // Variable declarations can have annotations
            if k == "variable_statement"
                || k == "export_variable_statement"
                || k == "onready_variable_statement"
                || k == "decorated_definition"
            {
                let start = node.start_byte();
                if flagged_positions.contains(&start) {
                    return;
                }
                let text = node_text(node, src);
                let has_onready = text.contains("@onready");
                let has_export = text.contains("@export");
                if has_onready && has_export {
                    flagged_positions.insert(start);
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        message: "Do not combine @onready with @export on the same variable."
                            .to_string(),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(node),
                        notes: vec![Note {
                            message: "The @onready default value will override the exported value after _ready().".to_string(),
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
