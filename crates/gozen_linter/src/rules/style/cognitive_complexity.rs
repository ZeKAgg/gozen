use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};
use crate::rules::complexity::compute_cognitive_for_function;

const MAX_COGNITIVE_COMPLEXITY: usize = 15;

pub struct CognitiveComplexity;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/cognitiveComplexity",
    name: "cognitiveComplexity",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Function cognitive complexity is too high.",
    explanation: "Cognitive complexity increases with branching, nesting depth, flow interruptions (break/continue), and boolean chains in conditions. Lower scores improve readability and maintenance. Default threshold: 15.",
};

impl Rule for CognitiveComplexity {
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

            let complexity = compute_cognitive_for_function(node, src);
            if complexity <= MAX_COGNITIVE_COMPLEXITY {
                return;
            }

            let name = first_identifier_child(node)
                .map(|n| node_text(n, src))
                .unwrap_or("<anonymous>");
            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: format!(
                    "Function \"{}\" has cognitive complexity {} (maximum is {}).",
                    name, complexity, MAX_COGNITIVE_COMPLEXITY
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
    use super::CognitiveComplexity;
    use crate::rule::Rule;
    use gozen_parser::GDScriptParser;

    #[test]
    fn no_diagnostic_below_threshold() {
        let source = "func f():\n\tif a:\n\t\tpass\n";
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("source parses");
        let diags = CognitiveComplexity.check(&tree, source, None);
        assert!(diags.is_empty());
    }

    #[test]
    fn emits_diagnostic_above_threshold() {
        let source = r#"func f():
	if a and b and c:
		while run:
			if deep:
				for i in range(5):
					if stop:
						break
			elif alt:
				while x:
					if y:
						continue
"#;
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("source parses");
        let diags = CognitiveComplexity.check(&tree, source, None);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("cognitive complexity"));
    }
}
