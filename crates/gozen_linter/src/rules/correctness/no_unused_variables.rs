use gozen_diagnostics::{Diagnostic, Fix, Severity, TextEdit};
use gozen_parser::{
    first_identifier_child, node_text, span_from_node, walk_tree, Node, Span, Tree,
};

use crate::rule::{Rule, RuleMetadata};

pub struct NoUnusedVariables;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noUnusedVariables",
    name: "noUnusedVariables",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: true,
    description: "Variables declared with var that are never read.",
    explanation: "Declaring a variable that is never used adds noise. Remove it or prefix with _ to indicate intentionally unused.",
};

impl Rule for NoUnusedVariables {
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
        // NOTE: This rule uses file-global declaration/reference matching. Variables with
        // the same name in different functions can cancel each other out (false negatives).
        // A per-scope analysis would be more precise but is significantly more complex.
        let mut declared: Vec<(String, Span)> = Vec::new();
        let mut referenced: std::collections::HashSet<String> = std::collections::HashSet::new();

        walk_tree(root, source, |node, src| {
            if node.kind() == "variable_statement" {
                if let Some(name_node) = first_identifier_child(node) {
                    let name = node_text(name_node, src).to_string();
                    if !name.starts_with('_') {
                        let statement_span = span_from_node(node);
                        declared.push((name, statement_span));
                    }
                }
            }
        });

        walk_tree(root, source, |node, src| {
            if node.kind() == "identifier" && !is_declaration_name(node, tree, source) {
                referenced.insert(node_text(node, src).to_string());
            }
        });

        let mut diags = Vec::new();
        for (name, span) in declared {
            if !referenced.contains(&name) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Variable \"{}\" is declared but never used.", name),
                    file_path: None,
                    rule_id: None,
                    span,
                    notes: vec![],
                    fix: Some(Fix {
                        description: "Remove unused variable".into(),
                        is_safe: true,
                        changes: vec![TextEdit {
                            span,
                            new_text: String::new(),
                        }],
                    }),
                });
            }
        }
        diags
    }
}

fn is_declaration_name(ident_node: Node, _tree: &Tree, _source: &str) -> bool {
    let mut cursor = ident_node.walk();
    if !cursor.goto_parent() {
        return false;
    }
    let parent = cursor.node();
    if parent.kind() == "variable_statement" {
        for i in 0..parent.child_count() {
            if let Some(c) = parent.child(i) {
                if c.is_named() && c.start_byte() == ident_node.start_byte() {
                    return true;
                }
            }
        }
    }
    false
}
