use gozen_diagnostics::{Diagnostic, Note, Severity, Span};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};
use gozen_project::{ProjectGraph, SceneData};

use crate::rule::{ProjectRule, RuleMetadata};

pub struct InvalidResourceType;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/invalidResourceType",
    name: "invalidResourceType",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Exported variable type does not match assigned scene resource type.",
    explanation: "When a scene assigns an ExtResource/SubResource to an @export variable, the assigned resource type should be compatible with the exported type annotation.",
};

struct ExportedType {
    name: String,
    expected_type: String,
    span: Span,
}

impl ProjectRule for InvalidResourceType {
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

        let script = match graph.scripts.get(script_path) {
            Some(s) if !s.attached_to_scenes.is_empty() => s,
            _ => return diags,
        };

        let exported = collect_exported_types(tree, source);
        if exported.is_empty() {
            return diags;
        }

        for scene_path in &script.attached_to_scenes {
            let scene = match graph.scenes.get(scene_path) {
                Some(s) => s,
                None => continue,
            };

            for node in &scene.nodes {
                if node.script.as_deref() != Some(script_path) {
                    continue;
                }

                for exp in &exported {
                    let assigned_value = match node.properties.get(&exp.name) {
                        Some(v) => v.as_str(),
                        None => continue,
                    };
                    let actual_type = match resolve_assigned_resource_type(assigned_value, scene) {
                        Some(t) => t,
                        None => continue,
                    };

                    if !types_compatible(&exp.expected_type, &actual_type) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "Exported variable `{}` expects `{}` but scene assigns `{}`.",
                                exp.name, exp.expected_type, actual_type
                            ),
                            file_path: None,
                            rule_id: None,
                            span: exp.span,
                            notes: vec![Note {
                                message: format!(
                                    "Scene: {} (node: {})",
                                    scene.path, node.full_path
                                ),
                                span: None,
                            }],
                            fix: None,
                        });
                    }
                }
            }
        }

        diags
    }
}

fn collect_exported_types(tree: &Tree, source: &str) -> Vec<ExportedType> {
    let root = tree.root_node();
    let mut items = Vec::new();
    walk_tree(root, source, |node, src| {
        let kind = node.kind();
        if kind != "variable_statement"
            && kind != "export_variable_statement"
            && kind != "decorated_definition"
        {
            return;
        }
        let text = node_text(node, src);
        if !text.contains("@export") {
            return;
        }
        if let Some((name, expected_type)) = parse_exported_type(text) {
            items.push(ExportedType {
                name,
                expected_type,
                span: span_from_node(node),
            });
        }
    });
    items
}

fn parse_exported_type(text: &str) -> Option<(String, String)> {
    let var_pos = text.find("var ")?;
    let after_var = text[var_pos + 4..].trim();

    let name = after_var
        .split(|c: char| c.is_whitespace() || c == ':' || c == '=')
        .next()?
        .trim();
    if name.is_empty() {
        return None;
    }

    // Find a type-hint colon before the first assignment sign.
    let eq_pos = after_var.find('=').unwrap_or(after_var.len());
    let header = &after_var[..eq_pos];
    let colon = header.find(':')?;
    let type_src = header[colon + 1..].trim_start();
    if type_src.is_empty() {
        return None;
    }
    let expected_type = type_src
        .split(|c: char| c.is_whitespace() || c == '=' || c == '#')
        .next()?
        .trim();
    if expected_type.is_empty() {
        return None;
    }

    Some((name.to_string(), expected_type.to_string()))
}

fn resolve_assigned_resource_type(value: &str, scene: &SceneData) -> Option<String> {
    let raw = value.trim();
    if raw.eq_ignore_ascii_case("null") {
        return None;
    }

    if raw.starts_with("ExtResource(") {
        let id = extract_resource_id(raw)?;
        return scene
            .external_resources
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.resource_type.clone());
    }
    if raw.starts_with("SubResource(") {
        let id = extract_resource_id(raw)?;
        return scene
            .sub_resources
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.resource_type.clone());
    }

    None
}

fn extract_resource_id(value: &str) -> Option<String> {
    let start = value.find('(')?;
    let end = value.rfind(')')?;
    if end <= start {
        return None;
    }
    let inner = value[start + 1..end].trim();
    Some(inner.trim_matches('"').to_string())
}

fn types_compatible(expected: &str, actual: &str) -> bool {
    if expected == actual {
        return true;
    }
    if expected == "Resource" {
        return true;
    }
    if expected == "Texture" {
        return actual.contains("Texture");
    }
    if expected == "Material" {
        return actual.ends_with("Material") || actual.contains("Material");
    }
    false
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::rule::ProjectRule;
    use gozen_parser::{GDScriptParser, Tree};
    use gozen_project::{
        ProjectGraph, SceneData, SceneNode, SceneSubResource, ScriptAttachment, ScriptData,
    };

    #[test]
    fn parse_exported_type_simple() {
        let parsed = parse_exported_type("@export var icon: Texture2D").unwrap();
        assert_eq!(parsed.0, "icon");
        assert_eq!(parsed.1, "Texture2D");
    }

    #[test]
    fn parse_exported_type_requires_hint() {
        assert!(parse_exported_type("@export var icon = preload(\"res://x.tres\")").is_none());
    }

    #[test]
    fn abstract_compatibility_rules() {
        assert!(types_compatible("Resource", "Texture2D"));
        assert!(types_compatible("Texture", "CompressedTexture2D"));
        assert!(types_compatible("Material", "ShaderMaterial"));
        assert!(!types_compatible("Texture2D", "CompressedTexture2D"));
    }

    fn base_graph_with_script(script_source: &str) -> (ProjectGraph, Tree) {
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(script_source).expect("script should parse");

        let mut graph = ProjectGraph::default();
        graph.scripts.insert(
            "res://scripts/player.gd".to_string(),
            ScriptData {
                path: "res://scripts/player.gd".to_string(),
                class_name: Some("Player".to_string()),
                signals: Vec::new(),
                functions: vec!["_ready".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: vec!["res://scenes/player.tscn".to_string()],
                attached_nodes: vec![ScriptAttachment {
                    scene_path: "res://scenes/player.tscn".to_string(),
                    node_name: "Player".to_string(),
                    node_full_path: "Player".to_string(),
                    node_parent_path: ".".to_string(),
                }],
            },
        );

        let mut props = HashMap::new();
        props.insert("icon".to_string(), "ExtResource(\"2\")".to_string());
        props.insert("material".to_string(), "SubResource(\"1\")".to_string());
        props.insert("nullable".to_string(), "null".to_string());

        graph.scenes.insert(
            "res://scenes/player.tscn".to_string(),
            SceneData {
                path: "res://scenes/player.tscn".to_string(),
                nodes: vec![SceneNode {
                    name: "Player".to_string(),
                    node_type: "Node2D".to_string(),
                    parent: ".".to_string(),
                    full_path: "Player".to_string(),
                    script: Some("res://scripts/player.gd".to_string()),
                    instanced_scene: None,
                    properties: props,
                }],
                connections: Vec::new(),
                external_resources: vec![gozen_project::ExternalResource {
                    resource_type: "Texture2D".to_string(),
                    path: "res://art/icon.png".to_string(),
                    id: "2".to_string(),
                }],
                sub_resources: vec![SceneSubResource {
                    resource_type: "ShaderMaterial".to_string(),
                    id: "1".to_string(),
                }],
            },
        );

        (graph, tree)
    }

    #[test]
    fn exact_match_produces_no_diagnostic() {
        let source = "@export var icon: Texture2D";
        let (graph, tree) = base_graph_with_script(source);
        let diags = InvalidResourceType.check(&tree, source, &graph, "res://scripts/player.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn mismatch_produces_diagnostic() {
        let source = "@export var icon: Material";
        let (graph, tree) = base_graph_with_script(source);
        let diags = InvalidResourceType.check(&tree, source, &graph, "res://scripts/player.gd");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("expects `Material`"));
    }

    #[test]
    fn abstract_match_texture_is_allowed() {
        let source = "@export var icon: Texture";
        let (graph, tree) = base_graph_with_script(source);
        let diags = InvalidResourceType.check(&tree, source, &graph, "res://scripts/player.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn unresolved_and_null_assignments_are_skipped() {
        let source = "@export var nullable: Texture2D\n@export var missing: Texture2D";
        let (graph, tree) = base_graph_with_script(source);
        let diags = InvalidResourceType.check(&tree, source, &graph, "res://scripts/player.gd");
        assert!(diags.is_empty());
    }
}
