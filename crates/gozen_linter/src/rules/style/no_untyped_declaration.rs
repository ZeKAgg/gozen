use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

fn has_type_hint(var_text: &str) -> bool {
    let trimmed = var_text.trim();
    if let Some(eq) = trimmed.find('=') {
        trimmed[..eq].contains(':')
    } else {
        trimmed.contains(':')
    }
}

fn missing_return_type(node: gozen_parser::Node, source: &str) -> Option<gozen_diagnostics::Span> {
    let text = node_text(node, source);
    if text.contains("->") {
        return None;
    }
    Some(span_from_node(node))
}

pub struct NoUntypedDeclaration;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/noUntypedDeclaration",
    name: "noUntypedDeclaration",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Variables and parameters without type hints.",
    explanation: "Adding type hints improves clarity and catches errors earlier.",
};

impl Rule for NoUntypedDeclaration {
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
            if node.kind() == "variable_statement" {
                let text = node_text(node, src);
                if !has_type_hint(text) {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "Variable declaration should have a type hint.".into(),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(node),
                        notes: vec![],
                        fix: None,
                    });
                }
            } else if node.kind() == "function_definition" {
                if let Some(span) = missing_return_type(node, src) {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "Function should declare a return type.".into(),
                        file_path: None,
                        rule_id: None,
                        span,
                        notes: vec![],
                        fix: None,
                    });
                }
            }
        });
        diags
    }
}
