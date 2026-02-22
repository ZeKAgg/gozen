use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::Tree;
use gozen_project::ProjectGraph;

use crate::rule::{ProjectRule, RuleMetadata};

pub struct MissingClassName;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/missingClassName",
    name: "missingClassName",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Script extends a custom class_name that cannot be resolved.",
    explanation: "If a script extends a custom class_name, that class must be declared by some script in the project graph. Unresolvable class names usually indicate a rename or missing file.",
};

const BUILTIN_BASE_CLASSES: &[&str] = &[
    "Object",
    "RefCounted",
    "Resource",
    "Node",
    "Node2D",
    "Node3D",
    "Control",
    "CanvasItem",
    "Window",
    "SceneTree",
    "MainLoop",
    "Timer",
    "Camera2D",
    "Camera3D",
    "CharacterBody2D",
    "CharacterBody3D",
    "Area2D",
    "Area3D",
    "RigidBody2D",
    "RigidBody3D",
    "StaticBody2D",
    "StaticBody3D",
    "AnimatableBody2D",
    "AnimatableBody3D",
    "CollisionObject2D",
    "CollisionObject3D",
    "Sprite2D",
    "AnimatedSprite2D",
    "Label",
    "Button",
    "TextureRect",
    "PackedScene",
    "AnimationPlayer",
    "AudioStreamPlayer",
    "GPUParticles2D",
    "GPUParticles3D",
    "NavigationAgent2D",
    "NavigationAgent3D",
];

impl ProjectRule for MissingClassName {
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

        let extends_name = match extract_extends_identifier(source) {
            Some(name) => name,
            None => return diags,
        };

        if BUILTIN_BASE_CLASSES.contains(&extends_name.as_str()) {
            return diags;
        }

        if graph.class_names.contains_key(&extends_name) {
            return diags;
        }

        diags.push(Diagnostic {
            severity: Severity::Warning,
            message: format!(
                "Extends unresolved class_name `{}` (not found in project).",
                extends_name
            ),
            file_path: None,
            rule_id: None,
            span: gozen_parser::span_from_node(tree.root_node()),
            notes: vec![Note {
                message: format!("Attached to {} scene(s).", script.attached_to_scenes.len()),
                span: None,
            }],
            fix: None,
        });
        diags
    }
}

fn extract_extends_identifier(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("extends ") {
            let raw = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(':');
            if raw.is_empty() || raw.starts_with('"') || raw.starts_with('\'') {
                return None;
            }
            if raw.contains('/') || raw.contains('.') {
                return None;
            }
            if !is_bare_identifier(raw) {
                return None;
            }
            return Some(raw.to_string());
        }
    }
    None
}

fn is_bare_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::rule::ProjectRule;
    use gozen_parser::{GDScriptParser, Tree};
    use gozen_project::{ProjectGraph, SceneData, SceneNode, ScriptAttachment, ScriptData};

    #[test]
    fn extracts_custom_extends_identifier() {
        let src = "extends MyBase\nclass_name Foo";
        assert_eq!(extract_extends_identifier(src).as_deref(), Some("MyBase"));
    }

    #[test]
    fn ignores_path_extends() {
        let src = "extends \"res://scripts/base.gd\"";
        assert!(extract_extends_identifier(src).is_none());
    }

    #[test]
    fn ignores_non_identifier_extends() {
        let src = "extends foo.bar.Baz";
        assert!(extract_extends_identifier(src).is_none());
    }

    fn build_graph_with_script(source: &str) -> (ProjectGraph, Tree) {
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("script should parse");

        let mut graph = ProjectGraph::default();
        graph.scripts.insert(
            "res://scripts/child.gd".to_string(),
            ScriptData {
                path: "res://scripts/child.gd".to_string(),
                class_name: Some("Child".to_string()),
                signals: Vec::new(),
                functions: vec!["_ready".to_string()],
                exported_vars: Vec::new(),
                attached_to_scenes: vec!["res://scenes/main.tscn".to_string()],
                attached_nodes: vec![ScriptAttachment {
                    scene_path: "res://scenes/main.tscn".to_string(),
                    node_name: "Child".to_string(),
                    node_full_path: "Child".to_string(),
                    node_parent_path: ".".to_string(),
                }],
            },
        );
        graph.scenes.insert(
            "res://scenes/main.tscn".to_string(),
            SceneData {
                path: "res://scenes/main.tscn".to_string(),
                nodes: vec![SceneNode {
                    name: "Child".to_string(),
                    node_type: "Node".to_string(),
                    parent: ".".to_string(),
                    full_path: "Child".to_string(),
                    script: Some("res://scripts/child.gd".to_string()),
                    instanced_scene: None,
                    properties: HashMap::new(),
                }],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );
        (graph, tree)
    }

    #[test]
    fn built_in_extends_is_ignored() {
        let source = "extends Node2D";
        let (graph, tree) = build_graph_with_script(source);
        let diags = MissingClassName.check(&tree, source, &graph, "res://scripts/child.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn known_custom_class_extends_is_ignored() {
        let source = "extends CustomBase";
        let (mut graph, tree) = build_graph_with_script(source);
        graph.class_names.insert(
            "CustomBase".to_string(),
            "res://scripts/custom_base.gd".to_string(),
        );
        let diags = MissingClassName.check(&tree, source, &graph, "res://scripts/child.gd");
        assert!(diags.is_empty());
    }

    #[test]
    fn unknown_custom_class_emits_diagnostic() {
        let source = "extends UnknownBase";
        let (graph, tree) = build_graph_with_script(source);
        let diags = MissingClassName.check(&tree, source, &graph, "res://scripts/child.gd");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("UnknownBase"));
    }
}
