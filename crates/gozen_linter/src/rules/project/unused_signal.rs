use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};
use gozen_project::ProjectGraph;

use crate::rule::{ProjectRule, RuleMetadata};

pub struct UnusedSignal;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/unusedSignal",
    name: "unusedSignal",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Signal declared but never emitted or connected.",
    explanation: "A signal is declared in this script but is never emitted (via .emit() or emit_signal()) and never connected (via .connect() or scene connections). This may indicate dead code.",
};

impl ProjectRule for UnusedSignal {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        tree: &Tree,
        source: &str,
        graph: &ProjectGraph,
        script_path: &str,
    ) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        let script_data = match graph.scripts.get(script_path) {
            Some(s) => s,
            None => return diags,
        };

        if script_data.signals.is_empty() {
            return diags;
        }

        // Check which signals are used in the source code
        let mut used_signals = std::collections::HashSet::new();

        // Check source text for emit/connect patterns
        for signal_name in &script_data.signals {
            // Check for emission: signal_name.emit() or emit_signal("signal_name")
            if source.contains(&format!("{}.emit(", signal_name))
                || source.contains(&format!("{}.emit (", signal_name))
                || source.contains(&format!("emit_signal(\"{}\"", signal_name))
                || source.contains(&format!("emit_signal( \"{}\"", signal_name))
            {
                used_signals.insert(signal_name.clone());
                continue;
            }

            // Check for connection in code: signal_name.connect(
            if source.contains(&format!("{}.connect(", signal_name))
                || source.contains(&format!("{}.connect (", signal_name))
            {
                used_signals.insert(signal_name.clone());
                continue;
            }

            // Check for scene connections
            for scene_path in &script_data.attached_to_scenes {
                if let Some(scene) = graph.scenes.get(scene_path) {
                    for conn in &scene.connections {
                        if conn.signal == *signal_name {
                            used_signals.insert(signal_name.clone());
                        }
                    }
                }
            }
        }

        // Find signal declaration nodes in the AST
        let root = tree.root_node();
        walk_tree(root, source, |node, src| {
            if node.kind() == "signal_statement" {
                let text = node_text(node, src).trim();
                // Extract signal name: "signal foo" or "signal foo(params)"
                if let Some(rest) = text.strip_prefix("signal ") {
                    let name = rest
                        .split(|c: char| c.is_whitespace() || c == '(')
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !name.is_empty() && !used_signals.contains(name) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "Signal \"{}\" is declared but never emitted or connected.",
                                name
                            ),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(node),
                            notes: vec![Note {
                                message: "Remove the signal or add an emit/connect call."
                                    .to_string(),
                                span: None,
                            }],
                            fix: None,
                        });
                    }
                }
            }
        });

        diags
    }
}
