use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct ExpressionNotAssigned;

const METADATA: RuleMetadata = RuleMetadata {
    id: "suspicious/expressionNotAssigned",
    name: "expressionNotAssigned",
    group: "suspicious",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Standalone expression that has no side effect.",
    explanation: "An expression statement that does not assign a value, call a function, or use `await` likely has no effect and is probably a mistake (e.g., `x + 1` instead of `x += 1`, or a function name without `()`).",
};

impl Rule for ExpressionNotAssigned {
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

        walk_tree(root, source, |node, _src| {
            // Look for expression_statement nodes
            if node.kind() != "expression_statement" {
                return;
            }

            // Get the expression child
            let expr = match node.child(0) {
                Some(c) if c.is_named() => c,
                _ => return,
            };

            if !is_side_effect_free(expr) {
                return;
            }

            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: "Expression result is not used.".to_string(),
                file_path: None,
                rule_id: None,
                span: span_from_node(node),
                notes: vec![],
                fix: None,
            });
        });

        diags
    }
}

/// Returns `true` if the expression node (and all its children) have no side effects.
/// Recursively checks children so that `func1() + func2()` or `(some_call())` are
/// correctly identified as having side effects.
fn is_side_effect_free(node: Node) -> bool {
    let kind = node.kind();
    match kind {
        // Function calls have side effects
        "call" | "call_expression" | "method_call" => false,
        // Await expressions have side effects
        "await_expression" | "await" => false,
        // Assignment-like expressions have side effects
        "assignment"
        | "assignment_expression"
        | "augmented_assignment"
        | "augmented_assignment_expression" => false,
        // Yield has side effects
        "yield_expression" | "yield" => false,
        // Leaf nodes with no side effects
        "identifier" | "integer" | "float" | "string" | "true" | "false" | "null" | "nil" => true,
        // Compound expressions: must recursively check children
        "binary_operator"
        | "binary_expression"
        | "comparison_operator"
        | "comparison_expression"
        | "unary_operator"
        | "unary_expression"
        | "not_operator"
        | "boolean_operator"
        | "boolean_expression"
        | "parenthesized_expression"
        | "array"
        | "dictionary"
        | "subscript"
        | "subscript_expression"
        | "conditional_expression"
        | "ternary_expression"
        | "concatenated_string"
        | "attribute" => {
            // Check all named children recursively
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.is_named() && !is_side_effect_free(child) {
                        return false;
                    }
                }
            }
            true
        }
        // For anything else, assume it might have side effects (conservative)
        _ => false,
    }
}
