use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Tree};
use gozen_project::ProjectGraph;

use crate::rule::{ProjectRule, RuleMetadata};

pub struct MissingSignalHandler;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/missingSignalHandler",
    name: "missingSignalHandler",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Signal connected to a method that does not exist.",
    explanation: "Signals connected in .tscn or in code must target an existing function.",
};

fn function_names_in_script(tree: &Tree, source: &str) -> std::collections::HashSet<String> {
    let root = tree.root_node();
    let mut names = std::collections::HashSet::new();
    walk_tree(root, source, |node, src| {
        if node.kind() == "function_definition" {
            if let Some(name_node) = first_identifier_child(node) {
                names.insert(node_text(name_node, src).to_string());
            }
        }
    });
    names
}

fn required_signal_handlers(graph: &ProjectGraph, script_path: &str) -> Vec<(String, String)> {
    let mut required = Vec::new();
    let script_data = match graph.scripts.get(script_path) {
        Some(s) => s,
        None => return required,
    };
    for scene_path in &script_data.attached_to_scenes {
        let scene = match graph.scenes.get(scene_path) {
            Some(s) => s,
            None => continue,
        };
        for conn in &scene.connections {
            let to_script = resolve_connection_target_script(&scene.nodes, &conn.to_node);
            if to_script.as_deref() == Some(script_path) {
                required.push((conn.signal.clone(), conn.method.clone()));
            }
        }
    }
    required
}

fn resolve_connection_target_script(
    nodes: &[gozen_project::SceneNode],
    to_node: &str,
) -> Option<String> {
    if to_node == "." || to_node.is_empty() {
        return nodes
            .iter()
            .find(|n| n.parent == "." || n.parent.is_empty())
            .and_then(|n| n.script.clone());
    }
    nodes
        .iter()
        .find(|n| n.full_path == *to_node || n.name == *to_node)
        .and_then(|n| n.script.clone())
}

impl ProjectRule for MissingSignalHandler {
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
        let defined = function_names_in_script(tree, source);
        let required = required_signal_handlers(graph, script_path);
        let mut diags = Vec::new();
        for (signal, method) in required {
            if !defined.contains(&method) {
                let root = tree.root_node();
                let span = span_from_node(root);
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "Signal \"{}\" is connected to \"{}\" but no such function exists.",
                        signal, method
                    ),
                    file_path: None,
                    rule_id: None,
                    span,
                    notes: vec![],
                    fix: None,
                });
            }
        }
        diags
    }
}
