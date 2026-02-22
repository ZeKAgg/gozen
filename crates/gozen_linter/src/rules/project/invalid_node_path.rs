use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};
use gozen_project::ProjectGraph;

use crate::rule::{ProjectRule, RuleMetadata};

pub struct InvalidNodePath;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/invalidNodePath",
    name: "invalidNodePath",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Node path does not exist in the attached scene.",
    explanation: "A $NodePath or get_node() reference points to a node that does not exist in the scene tree attached to this script. This will cause a runtime error.",
};

impl ProjectRule for InvalidNodePath {
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

        // Collect all node paths available in scenes this script is attached to
        let script_data = match graph.scripts.get(script_path) {
            Some(s) => s,
            None => return diags,
        };

        let mut available_paths: Vec<String> = Vec::new();
        for scene_path in &script_data.attached_to_scenes {
            if let Some(scene) = graph.scenes.get(scene_path) {
                for node in &scene.nodes {
                    available_paths.push(node.full_path.clone());
                    available_paths.push(node.name.clone());
                }
            }
        }

        if available_paths.is_empty() {
            // Script not attached to any scene — can't validate
            return diags;
        }

        let root = tree.root_node();
        walk_tree(root, source, |node, src| {
            // Check for $NodePath expressions (node_path, get_node_expression, etc.)
            let kind = node.kind();
            let text = node_text(node, src).trim();

            if kind == "get_node_expression" || kind == "node_path" {
                // $Foo/Bar or $"Foo/Bar"
                let path = extract_dollar_path(text);
                if !path.is_empty() {
                    check_path(&path, &available_paths, node, &mut diags);
                }
            } else if kind == "call_expression" || kind == "call" {
                // get_node("Foo/Bar")
                if let Some(path) = extract_get_node_path(node, src) {
                    check_path(&path, &available_paths, node, &mut diags);
                }
            }
        });

        diags
    }
}

/// Extract the node path from a $ expression like "$Foo/Bar" or $"Foo/Bar".
fn extract_dollar_path(text: &str) -> String {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix('$') {
        let rest = rest.trim_matches('"');
        return rest.to_string();
    }
    String::new()
}

/// Extract the path argument from a get_node("path") call.
fn extract_get_node_path<'a>(node: gozen_parser::Node<'a>, source: &'a str) -> Option<String> {
    let name = gozen_parser::call_name(node, source);
    if name != "get_node" {
        return None;
    }
    // Find the argument list child
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let kind = child.kind();
            if kind == "argument_list" || kind == "arguments" {
                // Get the first named child (should be a string literal)
                for j in 0..child.child_count() {
                    if let Some(arg) = child.child(j) {
                        if arg.is_named() {
                            let arg_text = node_text(arg, source).trim();
                            if arg_text.starts_with('"')
                                && arg_text.ends_with('"')
                                && arg_text.len() > 1
                            {
                                return Some(arg_text[1..arg_text.len() - 1].to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check whether a node path exists in the available paths from the scene.
fn check_path(
    path: &str,
    available_paths: &[String],
    node: gozen_parser::Node,
    diags: &mut Vec<Diagnostic>,
) {
    // Normalize: strip leading "/" if present
    let normalized = path.trim_start_matches('/');

    // Check if this matches any known node name or full path
    let matches = available_paths.iter().any(|p| {
        let p_normalized = p.trim_start_matches('/');
        p_normalized == normalized
            || p_normalized.ends_with(&format!("/{}", normalized))
            || p.as_str() == normalized
    });

    if !matches {
        // Find similar paths for suggestions
        let suggestions: Vec<&str> = available_paths
            .iter()
            .filter(|p| {
                let last = p.rsplit('/').next().unwrap_or(p);
                let target_last = normalized.rsplit('/').next().unwrap_or(normalized);
                last.to_lowercase().contains(&target_last.to_lowercase())
                    || target_last.to_lowercase().contains(&last.to_lowercase())
            })
            .map(|s| s.as_str())
            .take(3)
            .collect();

        let mut notes = Vec::new();
        if !suggestions.is_empty() {
            notes.push(gozen_diagnostics::Note {
                message: format!("Available similar nodes: {}", suggestions.join(", ")),
                span: None,
            });
        }

        diags.push(Diagnostic {
            severity: Severity::Error,
            message: format!(
                "Node path \"{}\" does not exist in the attached scene.",
                path
            ),
            file_path: None,
            rule_id: None,
            span: span_from_node(node),
            notes,
            fix: None,
        });
    }
}
