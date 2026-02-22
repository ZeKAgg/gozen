use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct ExportTypeHint;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/exportTypeHint",
    name: "exportTypeHint",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "@export variables should have explicit type hints.",
    explanation: "Exported variables require type hints for proper inspector integration in Godot 4. Without a type hint, the editor cannot display the correct widget.",
};

impl Rule for ExportTypeHint {
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
            let k = node.kind();
            // Catch variable statements that have @export annotation
            if k == "variable_statement"
                || k == "export_variable_statement"
                || k == "decorated_definition"
            {
                let text = node_text(node, src);
                if !text.contains("@export") {
                    return;
                }
                // Check if there's a type hint: look for `: Type` pattern before `=`
                if has_type_hint(text) {
                    return;
                }
                // Some @export variants imply types (e.g., @export_range, @export_enum)
                if text.contains("@export_range")
                    || text.contains("@export_enum")
                    || text.contains("@export_flags")
                    || text.contains("@export_exp_easing")
                    || text.contains("@export_color_no_alpha")
                    || text.contains("@export_node_path")
                    || text.contains("@export_file")
                    || text.contains("@export_dir")
                    || text.contains("@export_multiline")
                    || text.contains("@export_global_file")
                    || text.contains("@export_global_dir")
                    || text.contains("@export_tool_button")
                {
                    return;
                }

                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "@export variable should have an explicit type hint.".to_string(),
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

fn has_type_hint(text: &str) -> bool {
    // Look for `: Type` or `:=` pattern, but not inside strings.
    // A type hint appears as `var name: Type = value` or `var name: Type`
    // We need the colon to appear between `var name` and `=` (if present),
    // and NOT inside a string literal.
    let trimmed = text.trim();

    // Find the `var ` keyword position
    let var_pos = match trimmed.find("var ") {
        Some(p) => p + 4,
        None => return false,
    };
    let after_var = &trimmed[var_pos..];

    // Find the first `=` that's not inside a string, and any `:` before it
    let mut in_string = false;
    let mut string_char = '"';
    let mut prev = '\0';
    let mut found_colon = false;

    for c in after_var.chars() {
        if in_string {
            if c == string_char && prev != '\\' {
                in_string = false;
            }
        } else {
            match c {
                '"' | '\'' => {
                    in_string = true;
                    string_char = c;
                }
                ':' => {
                    found_colon = true;
                    break;
                }
                '=' => {
                    // Reached assignment without finding a colon — no type hint
                    return false;
                }
                _ => {}
            }
        }
        prev = c;
    }
    found_colon
}
