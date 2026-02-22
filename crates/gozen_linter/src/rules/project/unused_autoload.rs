use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};
use gozen_project::ProjectGraph;

use crate::rule::{ProjectRule, RuleMetadata};

pub struct UnusedAutoload;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/unusedAutoload",
    name: "unusedAutoload",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Autoload singleton registered but never referenced in any script.",
    explanation: "An autoload is registered in project.godot but no script references it by name. This may indicate dead configuration or a typo in the autoload name.",
};

impl ProjectRule for UnusedAutoload {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        tree: &Tree,
        source: &str,
        graph: &ProjectGraph,
        _script_path: &str,
    ) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Only report from the first script we encounter (avoid duplicate reports)
        // We check if this is the "first" script alphabetically
        if let Some(first_script) = graph.scripts.keys().min() {
            if _script_path != first_script.as_str() {
                return diags;
            }
        }

        // Collect all identifier names used across all scripts
        let mut all_identifiers = std::collections::HashSet::new();

        // Check the current script's identifiers via AST
        let root = tree.root_node();
        walk_tree(root, source, |node, src| {
            if node.kind() == "identifier" || node.kind() == "name" {
                let name = node_text(node, src).trim();
                if !name.is_empty() {
                    all_identifiers.insert(name.to_string());
                }
            }
        });

        // For other scripts, check exported signal/class names from the project graph
        // instead of reading files from disk during linting (which is slow)
        for (path, script_data) in &graph.scripts {
            if path == _script_path {
                continue; // Already handled via AST
            }
            // Use the already-parsed data from the project graph
            if let Some(ref class_name) = script_data.class_name {
                all_identifiers.insert(class_name.clone());
            }
            for sig in &script_data.signals {
                all_identifiers.insert(sig.clone());
            }
            for var in &script_data.exported_vars {
                all_identifiers.insert(var.name.clone());
            }
        }
        // Also check scene connections which reference autoload names
        for scene in graph.scenes.values() {
            for conn in &scene.connections {
                all_identifiers.insert(conn.from_node.clone());
                all_identifiers.insert(conn.to_node.clone());
            }
            for node in &scene.nodes {
                all_identifiers.insert(node.name.clone());
            }
        }

        // Check each autoload
        for autoload in &graph.autoloads {
            if !all_identifiers.contains(&autoload.name) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "Autoload \"{}\" is registered in project.godot but never referenced.",
                        autoload.name
                    ),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(tree.root_node()),
                    notes: vec![Note {
                        message: format!("Registered at: {}", autoload.path),
                        span: None,
                    }],
                    fix: None,
                });
            }
        }

        diags
    }
}
