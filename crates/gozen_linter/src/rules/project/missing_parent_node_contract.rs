use std::collections::{BTreeSet, HashSet};

use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::Tree;
use gozen_project::ProjectGraph;

use super::parent_contract_support::{
    extract_parent_node_paths, parent_has_child_path, resolve_parent_candidates_for_attachment,
};
use crate::rule::{ProjectRule, RuleMetadata};

pub struct MissingParentNodeContract;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/missingParentNodeContract",
    name: "missingParentNodeContract",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Child script expects parent node path contract that host parents do not provide.",
    explanation: "When a child scene script calls get_parent().get_node/has_node with a literal path, every resolved host parent that instances this scene must provide that path. Gozen reports only provable violations and skips ambiguous host placements.",
};

impl ProjectRule for MissingParentNodeContract {
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

        let contracts = extract_parent_node_paths(tree, source);
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
                    let Some(scene) = graph.scenes.get(&candidate.host_scene_path) else {
                        ambiguous = true;
                        break;
                    };
                    let Some(has_path) = parent_has_child_path(
                        scene,
                        &candidate.parent_node_full_path,
                        &contract.value,
                    ) else {
                        ambiguous = true;
                        break;
                    };
                    host_scenes.insert(candidate.host_scene_path.clone());
                    if has_path {
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
                        "Parent node contract missing: get_parent() does not provide node path \"{}\" in host scene(s): {}.",
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

    fn host_scene(parent_script: Option<&str>, include_required_node: bool) -> SceneData {
        let mut nodes = vec![
            SceneNode {
                name: "HostRoot".to_string(),
                node_type: "Node2D".to_string(),
                parent: ".".to_string(),
                full_path: "HostRoot".to_string(),
                script: parent_script.map(|s| s.to_string()),
                instanced_scene: None,
                properties: HashMap::new(),
            },
            SceneNode {
                name: "Child".to_string(),
                node_type: "Node2D".to_string(),
                parent: ".".to_string(),
                full_path: "HostRoot/Child".to_string(),
                script: None,
                instanced_scene: Some("res://scenes/child.tscn".to_string()),
                properties: HashMap::new(),
            },
        ];
        if include_required_node {
            nodes.push(SceneNode {
                name: "MustExist".to_string(),
                node_type: "Node2D".to_string(),
                parent: "HostRoot".to_string(),
                full_path: "HostRoot/MustExist".to_string(),
                script: None,
                instanced_scene: None,
                properties: HashMap::new(),
            });
        }
        SceneData {
            path: "res://scenes/host.tscn".to_string(),
            nodes,
            connections: Vec::new(),
            external_resources: Vec::new(),
            sub_resources: Vec::new(),
        }
    }

    #[test]
    fn missing_parent_node_contract_reports_when_all_hosts_missing() {
        let source = r#"func _ready():
    var n = get_parent().get_node("MustExist")
"#;
        let (mut graph, tree) = base_graph(source);
        graph.scenes.insert(
            "res://scenes/host.tscn".to_string(),
            host_scene(None, false),
        );
        let diags =
            MissingParentNodeContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("MustExist"));
    }

    #[test]
    fn missing_parent_node_contract_skips_when_parent_provides_node() {
        let source = r#"func _ready():
    var n = get_parent().get_node("MustExist")
"#;
        let (mut graph, tree) = base_graph(source);
        graph
            .scenes
            .insert("res://scenes/host.tscn".to_string(), host_scene(None, true));
        let diags =
            MissingParentNodeContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn missing_parent_node_contract_skips_dynamic_paths() {
        let source = r#"func _ready():
    var n = get_parent().get_node(path_value)
"#;
        let (mut graph, tree) = base_graph(source);
        graph.scenes.insert(
            "res://scenes/host.tscn".to_string(),
            host_scene(None, false),
        );
        let diags =
            MissingParentNodeContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn missing_parent_node_contract_skips_ambiguous_hosts() {
        let source = r#"func _ready():
    var n = get_parent().get_node("MustExist")
"#;
        let (mut graph, tree) = base_graph(source);
        graph.scenes.insert(
            "res://scenes/host_ambiguous.tscn".to_string(),
            SceneData {
                path: "res://scenes/host_ambiguous.tscn".to_string(),
                nodes: vec![SceneNode {
                    name: "Child".to_string(),
                    node_type: "Node2D".to_string(),
                    parent: ".".to_string(),
                    full_path: "Child".to_string(),
                    script: None,
                    instanced_scene: Some("res://scenes/child.tscn".to_string()),
                    properties: HashMap::new(),
                }],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );
        let diags =
            MissingParentNodeContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn missing_parent_node_contract_reports_for_non_root_attachment_when_missing() {
        let source = r#"func _ready():
    var n = get_parent().get_node("ExpectedUnderAnchor")
"#;
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("script should parse");

        let mut graph = ProjectGraph::default();
        graph.scripts.insert(
            "res://scripts/child_non_root.gd".to_string(),
            ScriptData {
                path: "res://scripts/child_non_root.gd".to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: vec!["_ready".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: vec!["res://scenes/child_non_root.tscn".to_string()],
                attached_nodes: vec![ScriptAttachment {
                    scene_path: "res://scenes/child_non_root.tscn".to_string(),
                    node_name: "Inner".to_string(),
                    node_full_path: "ChildRoot/Inner".to_string(),
                    node_parent_path: "ChildRoot".to_string(),
                }],
            },
        );
        graph.scenes.insert(
            "res://scenes/child_non_root.tscn".to_string(),
            SceneData {
                path: "res://scenes/child_non_root.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "ChildRoot".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "ChildRoot".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Inner".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "ChildRoot".to_string(),
                        full_path: "ChildRoot/Inner".to_string(),
                        script: Some("res://scripts/child_non_root.gd".to_string()),
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );
        graph.scenes.insert(
            "res://scenes/host_non_root_fail.tscn".to_string(),
            SceneData {
                path: "res://scenes/host_non_root_fail.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "Host".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "Host".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Anchor".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "Host/Anchor".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "ChildInstance".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "Anchor".to_string(),
                        full_path: "Host/Anchor/ChildInstance".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child_non_root.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );

        let diags = MissingParentNodeContract.check(
            &tree,
            source,
            &graph,
            "res://scripts/child_non_root.gd",
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("ExpectedUnderAnchor"));
    }

    #[test]
    fn missing_parent_node_contract_reports_for_parent_traversal_when_missing() {
        let source = r#"func _ready():
    var n = get_parent().get_node("../Sibling")
"#;
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("script should parse");

        let mut graph = ProjectGraph::default();
        graph.scripts.insert(
            "res://scripts/child_traversal.gd".to_string(),
            ScriptData {
                path: "res://scripts/child_traversal.gd".to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: vec!["_ready".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: vec!["res://scenes/child_traversal.tscn".to_string()],
                attached_nodes: vec![ScriptAttachment {
                    scene_path: "res://scenes/child_traversal.tscn".to_string(),
                    node_name: "Inner".to_string(),
                    node_full_path: "TraversalRoot/Inner".to_string(),
                    node_parent_path: "TraversalRoot".to_string(),
                }],
            },
        );
        graph.scenes.insert(
            "res://scenes/child_traversal.tscn".to_string(),
            SceneData {
                path: "res://scenes/child_traversal.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "TraversalRoot".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "TraversalRoot".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Inner".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "TraversalRoot".to_string(),
                        full_path: "TraversalRoot/Inner".to_string(),
                        script: Some("res://scripts/child_traversal.gd".to_string()),
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );
        graph.scenes.insert(
            "res://scenes/host_traversal_fail.tscn".to_string(),
            SceneData {
                path: "res://scenes/host_traversal_fail.tscn".to_string(),
                nodes: vec![
                    SceneNode {
                        name: "Host".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "Host".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Anchor".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "Host/Anchor".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "ChildInstance".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "Anchor".to_string(),
                        full_path: "Host/Anchor/ChildInstance".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child_traversal.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );

        let diags = MissingParentNodeContract.check(
            &tree,
            source,
            &graph,
            "res://scripts/child_traversal.gd",
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("../Sibling"));
    }

    #[test]
    fn missing_parent_node_contract_deduplicates_same_contract_for_multiple_attachments() {
        let source = r#"func _ready():
    get_parent().get_node("MustExist")
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
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostRoot".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "Child".to_string(),
                        node_type: "Node2D".to_string(),
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
            MissingParentNodeContract.check(&tree, source, &graph, "res://scripts/child.gd");
        assert_eq!(diags.len(), 1);
    }
}
