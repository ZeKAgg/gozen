use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, Tree};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;
use crate::shader_rules::complexity::compute_cyclomatic_for_function;

const MAX_CYCLOMATIC_COMPLEXITY: usize = 10;

pub struct CyclomaticComplexity;

impl ShaderRule for CyclomaticComplexity {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/cyclomaticComplexity",
            name: "cyclomaticComplexity",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Shader function cyclomatic complexity is too high.",
            explanation: "Cyclomatic complexity starts at 1 and increases for each decision point (if, loops, switch/case, ternary, and boolean decision chains). Keep shader functions focused and easier to test. Default threshold: 10.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();

        for i in 0..root.child_count() {
            let Some(node) = root.child(i) else {
                continue;
            };
            if node.kind() != "function_declaration" {
                continue;
            }
            let complexity = compute_cyclomatic_for_function(node, source);
            if complexity <= MAX_CYCLOMATIC_COMPLEXITY {
                continue;
            }
            let name = function_name(node, source);
            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: format!(
                    "Shader function \"{}\" has cyclomatic complexity {} (maximum is {}).",
                    name, complexity, MAX_CYCLOMATIC_COMPLEXITY
                ),
                file_path: None,
                rule_id: None,
                span: Span {
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    start_row: node.start_position().row,
                    start_col: node.start_position().column,
                    end_row: node.end_position().row,
                    end_col: node.end_position().column,
                },
                notes: vec![],
                fix: None,
            });
        }

        diags
    }
}

fn function_name(node: gozen_parser::Node, source: &str) -> String {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return node_text(child, source).to_string();
            }
        }
    }
    "<anonymous>".to_string()
}

#[cfg(test)]
mod tests {
    use super::CyclomaticComplexity;
    use crate::shader_rule::ShaderRule;
    use gozen_parser::GDShaderParser;

    #[test]
    fn no_diagnostic_below_threshold() {
        let source = r#"
shader_type spatial;
void f() {
    if (x) {
        ALBEDO = vec3(1.0);
    }
}
"#;
        let mut parser = GDShaderParser::new();
        let tree = parser.parse(source).expect("source parses");
        let diags = CyclomaticComplexity.check(&tree, source);
        assert!(diags.is_empty());
    }

    #[test]
    fn emits_diagnostic_above_threshold() {
        let source = r#"
shader_type spatial;
void f() {
    if (a) {}
    else if (b) {}
    else if (c) {}
    else if (d) {}
    else if (e) {}
    else if (f0) {}
    else if (g) {}
    else if (h) {}
    else if (i) {}
    else if (j) {}
}
"#;
        let mut parser = GDShaderParser::new();
        let tree = parser.parse(source).expect("source parses");
        let diags = CyclomaticComplexity.check(&tree, source);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("cyclomatic complexity"));
    }
}
