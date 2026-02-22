use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};
use crate::rules::complexity::compute_cyclomatic_for_function;

const MAX_CYCLOMATIC_COMPLEXITY: usize = 10;

pub struct CyclomaticComplexity;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/cyclomaticComplexity",
    name: "cyclomaticComplexity",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Function cyclomatic complexity is too high.",
    explanation: "Cyclomatic complexity starts at 1 and increases for each decision point (if/elif, loops, match/switch branches, ternary, and boolean decision chains). Keep functions small to improve readability and testability. Default threshold: 10.",
};

impl Rule for CyclomaticComplexity {
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

            let complexity = compute_cyclomatic_for_function(node, src);
            if complexity <= MAX_CYCLOMATIC_COMPLEXITY {
                return;
            }

            let name = first_identifier_child(node)
                .map(|n| node_text(n, src))
                .unwrap_or("<anonymous>");
            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: format!(
                    "Function \"{}\" has cyclomatic complexity {} (maximum is {}).",
                    name, complexity, MAX_CYCLOMATIC_COMPLEXITY
                ),
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

#[cfg(test)]
mod tests {
    use super::CyclomaticComplexity;
    use crate::rule::Rule;
    use gozen_parser::GDScriptParser;

    #[test]
    fn no_diagnostic_below_threshold() {
        let source = "func f():\n\tif a:\n\t\tpass\n";
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("source parses");
        let diags = CyclomaticComplexity.check(&tree, source, None);
        assert!(diags.is_empty());
    }

    #[test]
    fn emits_diagnostic_above_threshold() {
        let source = r#"func f():
	if a:
		pass
	elif b:
		pass
	elif c:
		pass
	elif d:
		pass
	elif e:
		pass
	elif f:
		pass
	elif g:
		pass
	elif h:
		pass
	elif i:
		pass
	elif j:
		pass
"#;
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("source parses");
        let diags = CyclomaticComplexity.check(&tree, source, None);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("cyclomatic complexity"));
    }
}
