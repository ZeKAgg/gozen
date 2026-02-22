use gozen_diagnostics::{Diagnostic, Note, Severity, Span};
use gozen_parser::Tree;
use gozen_project::ProjectGraph;

use crate::rule::{ProjectRule, RuleMetadata};

pub struct CyclicDependency;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/cyclicDependency",
    name: "cyclicDependency",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Circular dependency detected between scenes or resources.",
    explanation: "A scene instances another scene which eventually instances back to the original, creating a circular dependency. This will cause infinite recursion at load time or editor crashes.",
};

impl ProjectRule for CyclicDependency {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        _tree: &Tree,
        _source: &str,
        graph: &ProjectGraph,
        script_path: &str,
    ) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Only run cycle detection once per project — attach diagnostics to scripts
        // that are in scenes involved in cycles. To avoid duplicate reports, only
        // report if the current script's attached scene is the first in the cycle.
        let cycles = graph.detect_cycles();

        for cycle in &cycles {
            if cycle.is_empty() {
                continue;
            }

            let first_scene = &cycle[0];

            // Check if this script is attached to the first scene in the cycle
            if let Some(scene_data) = graph.scenes.get(first_scene) {
                let script_attached = scene_data
                    .nodes
                    .iter()
                    .any(|n| n.script.as_deref() == Some(script_path));

                if script_attached {
                    let cycle_str = cycle.join(" -> ");
                    let msg = format!("Circular dependency detected: {}", cycle_str);

                    let notes = vec![Note {
                        message: "This will cause infinite recursion at load time.".to_string(),
                        span: None,
                    }];

                    diags.push(Diagnostic {
                        message: msg,
                        span: Span {
                            start_byte: 0,
                            end_byte: 1,
                            start_row: 0,
                            start_col: 0,
                            end_row: 0,
                            end_col: 1,
                        },
                        severity: METADATA.default_severity,
                        file_path: Some(script_path.to_string()),
                        rule_id: Some(METADATA.id.to_string()),
                        fix: None,
                        notes,
                    });
                }
            }
        }

        diags
    }
}
