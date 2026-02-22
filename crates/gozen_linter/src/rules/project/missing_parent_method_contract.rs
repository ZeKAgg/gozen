use std::collections::{BTreeSet, HashSet};

use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::Tree;
use gozen_project::ProjectGraph;

use super::parent_contract_support::{
    extract_parent_method_names, parent_has_method, resolve_parent_candidates_for_attachment,
};
use crate::rule::{ProjectRule, RuleMetadata};

pub struct MissingParentMethodContract;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/missingParentMethodContract",
    name: "missingParentMethodContract",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Child script expects parent method contract that host parents do not provide.",
    explanation: "When a child scene script calls get_parent().method(...) or get_parent().call(\"method\", ...), each resolved host parent script must define that method. Gozen reports only provable violations and skips ambiguous placements.",
};

impl ProjectRule for MissingParentMethodContract {
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
        let mut emitted = HashSet::new();
        let Some(script) = graph.scripts.get(script_path) else {
            return diags;
        };
        let attachments: Vec<_> = script.attached_nodes.iter().collect();
        if attachments.is_empty() {
            return diags;
        }

        let contracts = extract_parent_method_names(tree, source);
        for contract in contracts {
            for attachment in &attachments {
                let Some(candidates) = resolve_parent_candidates_for_attachment(graph, attachment)
                else {
                    continue;
                };

                let mut missing_everywhere = true;
                let mut ambiguous = false;
                let mut host_scenes = BTreeSet::new();

                for candidate in &candidates {
                    host_scenes.insert(candidate.host_scene_path.clone());
                    let Some(has_method) = parent_has_method(graph, candidate, &contract.value)
                    else {
                        ambiguous = true;
                        break;
                    };
                    if has_method {
                        missing_everywhere = false;
                        break;
                    }
                }

                if ambiguous || !missing_everywhere {
                    continue;
                }
                if host_scenes.is_empty() {
                    continue;
                }
                let host_list: Vec<_> = host_scenes.into_iter().collect();
                let host_key = host_list.join(",");
                let dedup_key = format!(
                    "{}:{}:{}:{}",
                    contract.value, contract.span.start_byte, contract.span.end_byte, host_key
                );
                if !emitted.insert(dedup_key) {
                    continue;
                }

                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!(
                        "Parent method contract missing: get_parent().{}(...) is not defined in host parent script(s) for scene(s): {}.",
                        contract.value,
                        host_list.join(", ")
                    ),
                    file_path: None,
                    rule_id: None,
                    span: contract.span,
                    notes: Vec::new(),
                    fix: None,
                });
            }
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use gozen_parser::GDScriptParser;
    use gozen_project::{ProjectGraph, SceneData, SceneNode, ScriptAttachment, ScriptData};

    use super::*;

    fn base_graph(source: &str) -> (ProjectGraph, Tree) {
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("script should parse");
        let mut graph = ProjectGraph::default();
        graph.scripts.insert(
            "res://scripts/child.gd".to_string(),
            ScriptData {
                path: "res://scripts/child.gd".to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: vec!["_ready".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: vec!["res://scenes/child.tscn".to_string()],
                attached_nodes: vec![ScriptAttachment {
                    scene_path: "res://scenes/child.tscn".to_string(),
                    node_name: "ChildRoot".to_string(),
                    node_full_path: "ChildRoot".to_string(),
                    node_parent_path: ".".to_string(),
                }],
            },
        );
        (graph, tree)
    }

    fn with_host(
        mut graph: ProjectGraph,
        parent_script_res: &str,
        parent_functions: Vec<&str>,
    ) -> ProjectGraph {
        graph.scripts.insert(
            parent_script_res.to_string(),
            ScriptData {
                path: parent_script_res.to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: parent_functions
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect(),
                exported_vars: Vec::new(),
                attached_to_scenes: Vec::new(),
                attached_nodes: Vec::new(),
            },
        );
        graph.scenes.insert(
            "res://scenes/host.tscn".to_string(),
            SceneData {
                path: "res://scenes/host.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "HostRoot".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostRoot".to_string(),
                        script: Some(parent_script_res.to_string()),
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Child".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostRoot/Child".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );
        graph
    }

    #[test]
    fn missing_parent_method_contract_reports_when_missing_everywhere() {
        let source = r#"func _ready():
    get_parent().do_thing()
"#;
        let (graph, tree) = base_graph(source);
        let graph = with_host(graph, "res://scripts/parent.gd", vec!["_ready"]);
        let diags =
            MissingParentMethodContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("do_thing"));
    }

    #[test]
    fn missing_parent_method_contract_skips_when_parent_method_exists() {
        let source = r#"func _ready():
    get_parent().do_thing()
"#;
        let (graph, tree) = base_graph(source);
        let graph = with_host(graph, "res://scripts/parent.gd", vec!["_ready", "do_thing"]);
        let diags =
            MissingParentMethodContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn missing_parent_method_contract_supports_call_with_literal_name() {
        let source = r#"func _ready():
    get_parent().call("do_thing", 1, 2)
"#;
        let (graph, tree) = base_graph(source);
        let graph = with_host(graph, "res://scripts/parent.gd", vec!["_ready"]);
        let diags =
            MissingParentMethodContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("do_thing"));
    }

    #[test]
    fn missing_parent_method_contract_skips_ambiguous_host_set() {
        let source = r#"func _ready():
    get_parent().do_thing()
"#;
        let (mut graph, tree) = base_graph(source);
        graph.scripts.insert(
            "res://scripts/parent_ok.gd".to_string(),
            ScriptData {
                path: "res://scripts/parent_ok.gd".to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: vec!["_ready".to_string(), "do_thing".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: Vec::new(),
                attached_nodes: Vec::new(),
            },
        );
        graph.scripts.insert(
            "res://scripts/parent_bad.gd".to_string(),
            ScriptData {
                path: "res://scripts/parent_bad.gd".to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: vec!["_ready".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: Vec::new(),
                attached_nodes: Vec::new(),
            },
        );
        graph.scenes.insert(
            "res://scenes/host_ok.tscn".to_string(),
            SceneData {
                path: "res://scenes/host_ok.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "HostOk".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostOk".to_string(),
                        script: Some("res://scripts/parent_ok.gd".to_string()),
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Child".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostOk/Child".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );
        graph.scenes.insert(
            "res://scenes/host_alt.tscn".to_string(),
            SceneData {
                path: "res://scenes/host_alt.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "HostAlt".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostAlt".to_string(),
                        script: Some("res://scripts/parent_bad.gd".to_string()),
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Child".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostAlt/Child".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );
        let diags =
            MissingParentMethodContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn missing_parent_method_contract_deduplicates_same_contract_for_multiple_attachments() {
        let source = r#"func _ready():
    get_parent().do_thing()
"#;
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("script should parse");
        let mut graph = ProjectGraph::default();
        graph.scripts.insert(
            "res://scripts/child.gd".to_string(),
            ScriptData {
                path: "res://scripts/child.gd".to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: vec!["_ready".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: vec!["res://scenes/child.tscn".to_string()],
                attached_nodes: vec![
                    ScriptAttachment {
                        scene_path: "res://scenes/child.tscn".to_string(),
                        node_name: "Root".to_string(),
                        node_full_path: "Root".to_string(),
                        node_parent_path: ".".to_string(),
                    },
                    ScriptAttachment {
                        scene_path: "res://scenes/child.tscn".to_string(),
                        node_name: "AltRoot".to_string(),
                        node_full_path: "AltRoot".to_string(),
                        node_parent_path: ".".to_string(),
                    },
                ],
            },
        );
        graph.scenes.insert(
            "res://scenes/host.tscn".to_string(),
            SceneData {
                path: "res://scenes/host.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "HostRoot".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostRoot".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Child".to_string(),
                        node_type: "Node".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostRoot/Child".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );

        let diags =
            MissingParentMethodContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert_eq!(diags.len(), 1);
    }
}
