use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, Tree};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

/// Types that could benefit from lower precision hints.
const HIGHP_TYPES: &[&str] = &["float", "vec2", "vec3", "vec4", "mat2", "mat3", "mat4"];

pub struct PrecisionHints;

impl ShaderRule for PrecisionHints {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/precisionHints",
            name: "precisionHints",
            group: "shader",
            default_severity: Severity::Warning,
            has_fix: false,
            description: "Consider using lowp/mediump precision hints for uniforms.",
            explanation: "Using lower precision (lowp, mediump) for uniforms and varyings can improve performance on mobile GPUs. This is an opt-in rule for performance-sensitive shaders.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();

        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "uniform_declaration" || child.kind() == "varying_declaration" {
                    let text = node_text(child, source);
                    let trimmed = text.trim();
                    // Check if precision qualifier is already present
                    if trimmed.contains("lowp")
                        || trimmed.contains("mediump")
                        || trimmed.contains("highp")
                    {
                        continue;
                    }
                    // Check if the type is one that could use precision hints
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let type_name = parts[1];
                        if HIGHP_TYPES.contains(&type_name) {
                            diags.push(Diagnostic {
                                severity: Severity::Warning,
                                message: format!(
                                    "Consider adding a precision hint (lowp/mediump) for `{}` type.",
                                    type_name
                                ),
                                file_path: None,
                                rule_id: None,
                                span: Span {
                                    start_byte: child.start_byte(),
                                    end_byte: child.end_byte(),
                                    start_row: child.start_position().row,
                                    start_col: child.start_position().column,
                                    end_row: child.end_position().row,
                                    end_col: child.end_position().column,
                                },
                                notes: Vec::new(),
                                fix: None,
                            });
                        }
                    }
                }
            }
        }
        diags
    }
}
