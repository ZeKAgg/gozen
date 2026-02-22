use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

/// Default maximum number of function parameters.
const DEFAULT_MAX_ARGS: usize = 10;

pub struct FunctionArgumentsNumber;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/functionArgumentsNumber",
    name: "functionArgumentsNumber",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Function has too many parameters.",
    explanation: "Functions with many parameters are harder to understand and maintain. Consider using a Dictionary, a data class, or refactoring into smaller functions. Default threshold: 10.",
};

impl Rule for FunctionArgumentsNumber {
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
            if node.kind() != "function_definition" {
                return;
            }

            let func_name = first_identifier_child(node)
                .map(|n| node_text(n, src))
                .unwrap_or("<anonymous>");

            // Count parameters
            let param_count = count_parameters(node);

            if param_count > DEFAULT_MAX_ARGS {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "Function \"{}\" has {} parameters (maximum is {}).",
                        func_name, param_count, DEFAULT_MAX_ARGS
                    ),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(node),
                    notes: vec![],
                    fix: None,
                });
            }
        });

        diags
    }
}

fn count_parameters(func_node: gozen_parser::Node) -> usize {
    // Look for a parameters node child
    for i in 0..func_node.child_count() {
        if let Some(child) = func_node.child(i) {
            let kind = child.kind();
            if kind == "parameters" || kind == "parameter_list" {
                // Count named children that represent individual parameters
                let mut count = 0;
                for j in 0..child.child_count() {
                    if let Some(param) = child.child(j) {
                        if param.is_named() {
                            let pk = param.kind();
                            // Count parameter-like nodes (identifiers, typed parameters, default params)
                            if pk == "identifier"
                                || pk == "typed_parameter"
                                || pk == "default_parameter"
                                || pk == "parameter"
                                || pk == "name"
                            {
                                count += 1;
                            }
                        }
                    }
                }
                return count;
            }
        }
    }
    0
}
