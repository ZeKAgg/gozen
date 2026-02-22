use std::collections::BTreeSet;

use gozen_diagnostics::Span;
use gozen_parser::Tree;
use gozen_project::{ProjectGraph, SceneData, SceneNode, ScriptAttachment};

#[derive(Clone, Debug)]
pub struct ParentCandidate {
    pub host_scene_path: String,
    pub parent_node_full_path: String,
    pub parent_node_type: String,
    pub parent_script_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExtractedContract {
    pub value: String,
    pub span: Span,
}

pub fn resolve_parent_candidates_for_attachment(
    graph: &ProjectGraph,
    attachment: &ScriptAttachment,
) -> Option<Vec<ParentCandidate>> {
    let child_scene = graph.scenes.get(&attachment.scene_path);
    let child_root = child_scene.and_then(resolve_scene_root);

    let mut candidates: Vec<ParentCandidate> = Vec::new();
    let mut ambiguous = false;

    for (host_scene_path, host_scene) in &graph.scenes {
        for instance_node in &host_scene.nodes {
            if instance_node.instanced_scene.as_deref() != Some(attachment.scene_path.as_str()) {
                continue;
            }

            let parent_node =
                if attachment.node_parent_path.is_empty() || attachment.node_parent_path == "." {
                    match resolve_parent_node_of_instance(host_scene, instance_node) {
                        Some(n) => n,
                        None => {
                            ambiguous = true;
                            continue;
                        }
                    }
                } else {
                    let Some(child_scene) = child_scene else {
                        ambiguous = true;
                        continue;
                    };
                    let Some(child_root) = child_root else {
                        ambiguous = true;
                        continue;
                    };
                    let child_parent =
                        match resolve_scene_node_ref(child_scene, &attachment.node_parent_path) {
                            ResolveState::Unique(n) => n,
                            _ => {
                                ambiguous = true;
                                continue;
                            }
                        };
                    let mapped_parent_full = match map_child_full_path_to_host(
                        &child_root.full_path,
                        &instance_node.full_path,
                        &child_parent.full_path,
                    ) {
                        Some(p) => p,
                        None => {
                            ambiguous = true;
                            continue;
                        }
                    };
                    match resolve_scene_node_ref(host_scene, &mapped_parent_full) {
                        ResolveState::Unique(n) => n,
                        _ => {
                            ambiguous = true;
                            continue;
                        }
                    }
                };

            candidates.push(ParentCandidate {
                host_scene_path: host_scene_path.clone(),
                parent_node_full_path: parent_node.full_path.clone(),
                parent_node_type: parent_node.node_type.clone(),
                parent_script_path: parent_node.script.clone(),
            });
        }
    }

    if ambiguous || candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| {
        (
            a.host_scene_path.as_str(),
            a.parent_node_full_path.as_str(),
            a.parent_node_type.as_str(),
            a.parent_script_path.as_deref().unwrap_or(""),
        )
            .cmp(&(
                b.host_scene_path.as_str(),
                b.parent_node_full_path.as_str(),
                b.parent_node_type.as_str(),
                b.parent_script_path.as_deref().unwrap_or(""),
            ))
    });
    candidates.dedup_by(|a, b| {
        a.host_scene_path == b.host_scene_path
            && a.parent_node_full_path == b.parent_node_full_path
            && a.parent_node_type == b.parent_node_type
            && a.parent_script_path == b.parent_script_path
    });

    Some(candidates)
}

pub fn parent_has_signal(
    graph: &ProjectGraph,
    candidate: &ParentCandidate,
    signal: &str,
) -> Option<bool> {
    if let Some(script_path) = &candidate.parent_script_path {
        let script = graph.scripts.get(script_path)?;
        return Some(script.signals.iter().any(|s| s == signal));
    }
    has_builtin_signal(&candidate.parent_node_type, signal)
}

pub fn parent_has_method(
    graph: &ProjectGraph,
    candidate: &ParentCandidate,
    method: &str,
) -> Option<bool> {
    if let Some(script_path) = &candidate.parent_script_path {
        let script = graph.scripts.get(script_path)?;
        return Some(script.functions.iter().any(|m| m == method));
    }
    has_builtin_method(&candidate.parent_node_type, method)
}

pub fn parent_has_child_path(
    scene: &SceneData,
    parent_full_path: &str,
    path: &str,
) -> Option<bool> {
    let normalized = path.trim();
    if normalized.is_empty() {
        return None;
    }
    if normalized.starts_with('/')
        || normalized.starts_with("%")
        || normalized.contains(":")
        || normalized.contains("//")
    {
        return None;
    }

    let root = resolve_scene_root(scene)?;
    let mut current = resolve_scene_node_ref(scene, parent_full_path)
        .unique()?
        .full_path
        .clone();

    let trimmed = normalized.trim_start_matches("./");
    if trimmed.is_empty() || trimmed == "." {
        return Some(true);
    }

    for seg in trimmed.split('/') {
        if seg.is_empty() {
            return None;
        }
        if seg == "." {
            continue;
        }
        if seg == ".." {
            if current == root.full_path {
                return None;
            }
            let parent = parent_of(&current)?;
            current = parent.to_string();
            continue;
        }

        match resolve_child_by_name(scene, &current, seg) {
            ResolveState::Unique(node) => current = node.full_path.clone(),
            ResolveState::Missing => return Some(false),
            ResolveState::Ambiguous => return None,
        }
    }

    Some(true)
}

pub fn extract_parent_node_paths(tree: &Tree, source: &str) -> Vec<ExtractedContract> {
    let _ = tree;
    extract_from_lines(source, |text| {
        for prefix in [
            "get_parent().get_node(",
            "get_parent().get_node_or_null(",
            "get_parent().has_node(",
        ] {
            if let Some(path) = extract_first_string_arg_anywhere(text, prefix) {
                return Some(path);
            }
        }
        None
    })
}

pub fn extract_parent_signal_names(tree: &Tree, source: &str) -> Vec<ExtractedContract> {
    let _ = tree;
    extract_from_lines(source, |text| {
        extract_first_string_arg_anywhere(text, "get_parent().connect(")
    })
}

pub fn extract_parent_method_names(tree: &Tree, source: &str) -> Vec<ExtractedContract> {
    let _ = tree;
    extract_from_lines(source, |text| {
        if let Some(method_name) = extract_first_string_arg_anywhere(text, "get_parent().call(") {
            return Some(method_name);
        }
        let idx = text.find("get_parent().")?;
        let rest = &text[idx + "get_parent().".len()..];
        let paren_idx = rest.find('(')?;
        let method = &rest[..paren_idx];
        if method.is_empty()
            || !method
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            || matches!(
                method,
                "get_node" | "get_node_or_null" | "has_node" | "connect" | "call"
            )
        {
            return None;
        }
        Some(method.to_string())
    })
}

enum ResolveState<'a> {
    Unique(&'a SceneNode),
    Missing,
    Ambiguous,
}

impl<'a> ResolveState<'a> {
    fn unique(self) -> Option<&'a SceneNode> {
        match self {
            ResolveState::Unique(n) => Some(n),
            _ => None,
        }
    }
}

fn resolve_scene_root(scene: &SceneData) -> Option<&SceneNode> {
    let mut roots = scene
        .nodes
        .iter()
        .filter(|n| (n.parent.is_empty() || n.parent == ".") && !n.full_path.contains('/'));
    let root = roots.next()?;
    if roots.next().is_some() {
        return None;
    }
    Some(root)
}

fn resolve_parent_node_of_instance<'a>(
    scene: &'a SceneData,
    instance_node: &SceneNode,
) -> Option<&'a SceneNode> {
    if instance_node.parent.is_empty() || instance_node.parent == "." {
        let root = resolve_scene_root(scene)?;
        if root.full_path == instance_node.full_path {
            return None;
        }
        return Some(root);
    }
    resolve_scene_node_ref(scene, &instance_node.parent).unique()
}

fn resolve_scene_node_ref<'a>(scene: &'a SceneData, node_ref: &str) -> ResolveState<'a> {
    let direct: Vec<&SceneNode> = scene
        .nodes
        .iter()
        .filter(|n| n.full_path == node_ref)
        .collect();
    if direct.len() == 1 {
        return ResolveState::Unique(direct[0]);
    }
    if direct.len() > 1 {
        return ResolveState::Ambiguous;
    }

    let by_name: Vec<&SceneNode> = scene
        .nodes
        .iter()
        .filter(|n| n.name == node_ref || n.full_path.ends_with(&format!("/{}", node_ref)))
        .collect();
    if by_name.len() == 1 {
        ResolveState::Unique(by_name[0])
    } else if by_name.is_empty() {
        ResolveState::Missing
    } else {
        ResolveState::Ambiguous
    }
}

fn resolve_child_by_name<'a>(
    scene: &'a SceneData,
    parent_full_path: &str,
    child_name: &str,
) -> ResolveState<'a> {
    let matches: Vec<&SceneNode> = scene
        .nodes
        .iter()
        .filter(|n| parent_of(&n.full_path) == Some(parent_full_path) && n.name == child_name)
        .collect();
    if matches.len() == 1 {
        ResolveState::Unique(matches[0])
    } else if matches.is_empty() {
        ResolveState::Missing
    } else {
        ResolveState::Ambiguous
    }
}

fn map_child_full_path_to_host(
    child_root: &str,
    instance_full: &str,
    child_target: &str,
) -> Option<String> {
    if child_target == child_root {
        return Some(instance_full.to_string());
    }
    let prefix = format!("{}/", child_root.trim_end_matches('/'));
    if let Some(rest) = child_target.strip_prefix(&prefix) {
        return Some(format!("{}/{}", instance_full.trim_end_matches('/'), rest));
    }
    None
}

fn parent_of(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(p, _)| p)
}

fn normalize_for_matching(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut quote = '"';
    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
            if ch == quote {
                in_string = false;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            in_string = true;
            quote = ch;
            out.push(ch);
            continue;
        }
        if ch == '#' {
            break;
        }
        if ch.is_whitespace() {
            continue;
        }
        out.push(ch);
    }
    out
}

fn extract_first_string_arg(text: &str, prefix: &str) -> Option<String> {
    let rest = text.strip_prefix(prefix)?;
    let mut chars = rest.chars();
    let quote = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn extract_first_string_arg_anywhere(text: &str, prefix: &str) -> Option<String> {
    let idx = text.find(prefix)?;
    extract_first_string_arg(&text[idx..], prefix)
}

fn extract_from_lines<F>(source: &str, mut extractor: F) -> Vec<ExtractedContract>
where
    F: FnMut(&str) -> Option<String>,
{
    let mut out = Vec::new();
    let mut offset = 0usize;
    for (row, raw_line) in source.lines().enumerate() {
        let normalized = normalize_for_matching(raw_line);
        if let Some(value) = extractor(&normalized) {
            out.push(ExtractedContract {
                value,
                span: Span {
                    start_byte: offset,
                    end_byte: offset + raw_line.len(),
                    start_row: row,
                    start_col: 0,
                    end_row: row,
                    end_col: raw_line.len(),
                },
            });
        }
        offset += raw_line.len() + 1;
    }
    out
}

fn has_builtin_signal(class_name: &str, signal: &str) -> Option<bool> {
    let chain = class_chain(class_name)?;
    let mut set = BTreeSet::new();
    for class in chain {
        for s in builtin_signals_for(class) {
            set.insert(*s);
        }
    }
    Some(set.contains(signal))
}

fn has_builtin_method(class_name: &str, method: &str) -> Option<bool> {
    let chain = class_chain(class_name)?;
    let mut set = BTreeSet::new();
    for class in chain {
        for m in builtin_methods_for(class) {
            set.insert(*m);
        }
    }
    Some(set.contains(method))
}

fn class_chain(class_name: &str) -> Option<Vec<&'static str>> {
    let chain = match class_name {
        "CharacterBody2D" => vec!["CharacterBody2D", "Node2D", "CanvasItem", "Node", "Object"],
        "Control" => vec!["Control", "CanvasItem", "Node", "Object"],
        "Node2D" => vec!["Node2D", "CanvasItem", "Node", "Object"],
        "CanvasItem" => vec!["CanvasItem", "Node", "Object"],
        "Node" => vec!["Node", "Object"],
        "Object" => vec!["Object"],
        _ => return None,
    };
    Some(chain)
}

fn builtin_signals_for(class_name: &str) -> &'static [&'static str] {
    match class_name {
        "Object" => &["script_changed"],
        "Node" => &[
            "ready",
            "tree_entered",
            "tree_exiting",
            "child_entered_tree",
            "child_exiting_tree",
            "renamed",
        ],
        "CanvasItem" => &["visibility_changed", "item_rect_changed", "draw"],
        "Control" => &["focus_entered", "focus_exited", "resized"],
        "Node2D" => &[],
        "CharacterBody2D" => &[],
        _ => &[],
    }
}

fn builtin_methods_for(class_name: &str) -> &'static [&'static str] {
    match class_name {
        "Object" => &[
            "free",
            "has_method",
            "call",
            "connect",
            "disconnect",
            "is_class",
        ],
        "Node" => &[
            "add_child",
            "remove_child",
            "get_node",
            "get_node_or_null",
            "has_node",
            "get_parent",
            "queue_free",
            "call_deferred",
            "get_tree",
            "find_child",
        ],
        "CanvasItem" => &["show", "hide", "queue_redraw"],
        "Node2D" => &["rotate", "translate", "look_at", "to_local", "to_global"],
        "Control" => &["grab_focus", "release_focus", "set_anchors_preset"],
        "CharacterBody2D" => &["move_and_slide", "get_last_motion", "is_on_floor"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use gozen_project::{ProjectGraph, SceneData, SceneNode, ScriptAttachment};

    use super::*;

    fn host_scene_for_paths() -> SceneData {
        SceneData {
            path: "res://scenes/host.tscn".to_string(),
            nodes: vec![
                SceneNode {
                    name: "Root".to_string(),
                    node_type: "Node2D".to_string(),
                    parent: ".".to_string(),
                    full_path: "Root".to_string(),
                    script: None,
                    instanced_scene: None,
                    properties: HashMap::new(),
                },
                SceneNode {
                    name: "A".to_string(),
                    node_type: "Node2D".to_string(),
                    parent: "Root".to_string(),
                    full_path: "Root/A".to_string(),
                    script: None,
                    instanced_scene: None,
                    properties: HashMap::new(),
                },
                SceneNode {
                    name: "B".to_string(),
                    node_type: "Node2D".to_string(),
                    parent: "Root".to_string(),
                    full_path: "Root/B".to_string(),
                    script: None,
                    instanced_scene: None,
                    properties: HashMap::new(),
                },
                SceneNode {
                    name: "Leaf".to_string(),
                    node_type: "Node2D".to_string(),
                    parent: "Root/A".to_string(),
                    full_path: "Root/A/Leaf".to_string(),
                    script: None,
                    instanced_scene: None,
                    properties: HashMap::new(),
                },
            ],
            connections: Vec::new(),
            external_resources: Vec::new(),
            sub_resources: Vec::new(),
        }
    }

    #[test]
    fn parent_path_supports_dot_and_relative() {
        let scene = host_scene_for_paths();
        assert_eq!(parent_has_child_path(&scene, "Root", "."), Some(true));
        assert_eq!(parent_has_child_path(&scene, "Root", "A/Leaf"), Some(true));
        assert_eq!(
            parent_has_child_path(&scene, "Root", "A/Missing"),
            Some(false)
        );
    }

    #[test]
    fn parent_path_supports_up_level_and_bounds() {
        let scene = host_scene_for_paths();
        assert_eq!(parent_has_child_path(&scene, "Root/A", "../B"), Some(true));
        assert_eq!(parent_has_child_path(&scene, "Root", "../B"), None);
    }

    #[test]
    fn parent_path_rejects_unsupported_forms() {
        let scene = host_scene_for_paths();
        assert_eq!(parent_has_child_path(&scene, "Root", "/root/X"), None);
        assert_eq!(parent_has_child_path(&scene, "Root", "%Player"), None);
    }

    #[test]
    fn builtin_methods_inherit_through_chain() {
        let mut g = ProjectGraph::default();
        let cand = ParentCandidate {
            host_scene_path: "res://scenes/host.tscn".to_string(),
            parent_node_full_path: "Root".to_string(),
            parent_node_type: "CharacterBody2D".to_string(),
            parent_script_path: None,
        };
        assert_eq!(parent_has_method(&g, &cand, "move_and_slide"), Some(true));
        assert_eq!(parent_has_method(&g, &cand, "get_node"), Some(true));
        assert_eq!(
            parent_has_method(&g, &cand, "definitely_missing"),
            Some(false)
        );
        g.scripts.clear();
    }

    #[test]
    fn builtin_signals_inherit_through_chain() {
        let cand = ParentCandidate {
            host_scene_path: "res://scenes/host.tscn".to_string(),
            parent_node_full_path: "Root".to_string(),
            parent_node_type: "Control".to_string(),
            parent_script_path: None,
        };
        let graph = ProjectGraph::default();
        assert_eq!(
            parent_has_signal(&graph, &cand, "focus_entered"),
            Some(true)
        );
        assert_eq!(parent_has_signal(&graph, &cand, "ready"), Some(true));
        assert_eq!(parent_has_signal(&graph, &cand, "missing_sig"), Some(false));
    }

    #[test]
    fn resolves_non_root_attachment_parent_candidates() {
        let mut graph = ProjectGraph::default();

        graph.scenes.insert(
            "res://scenes/child.tscn".to_string(),
            SceneData {
                path: "res://scenes/child.tscn".to_string(),
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
                        script: Some("res://scripts/child.gd".to_string()),
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
                        name: "Anchor".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostRoot/Anchor".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "ChildInstance".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "Anchor".to_string(),
                        full_path: "HostRoot/Anchor/ChildInstance".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "ExpectedUnderAnchor".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "Anchor/ChildInstance".to_string(),
                        full_path: "HostRoot/Anchor/ChildInstance/ExpectedUnderAnchor".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );

        let attachment = ScriptAttachment {
            scene_path: "res://scenes/child.tscn".to_string(),
            node_name: "Inner".to_string(),
            node_full_path: "ChildRoot/Inner".to_string(),
            node_parent_path: "ChildRoot".to_string(),
        };
        let candidates = resolve_parent_candidates_for_attachment(&graph, &attachment).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].host_scene_path, "res://scenes/host.tscn");
        assert_eq!(
            candidates[0].parent_node_full_path,
            "HostRoot/Anchor/ChildInstance"
        );
        let host = graph.scenes.get("res://scenes/host.tscn").unwrap();
        assert_eq!(
            parent_has_child_path(
                host,
                &candidates[0].parent_node_full_path,
                "ExpectedUnderAnchor"
            ),
            Some(true)
        );
        assert_eq!(
            parent_has_child_path(host, &candidates[0].parent_node_full_path, "../Sibling"),
            Some(false)
        );
    }

    #[test]
    fn resolves_non_root_attachment_returns_none_when_mapping_ambiguous() {
        let mut graph = ProjectGraph::default();

        graph.scenes.insert(
            "res://scenes/child.tscn".to_string(),
            SceneData {
                path: "res://scenes/child.tscn".to_string(),
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
                        script: Some("res://scripts/child.gd".to_string()),
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
                        name: "Anchor".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: ".".to_string(),
                        full_path: "HostRoot/Anchor".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "ChildInstance".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "Anchor".to_string(),
                        full_path: "HostRoot/Anchor/ChildInstance".to_string(),
                        script: None,
                        instanced_scene: Some("res://scenes/child.tscn".to_string()),
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "ParentA".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "Anchor/ChildInstance".to_string(),
                        full_path: "HostRoot/Anchor/ChildInstance/ParentA".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                    SceneNode {
                        name: "ParentA".to_string(),
                        node_type: "Node2D".to_string(),
                        parent: "Anchor/ChildInstance".to_string(),
                        full_path: "HostRoot/Anchor/ChildInstance/OtherBranch/ParentA".to_string(),
                        script: None,
                        instanced_scene: None,
                        properties: HashMap::new(),
                    },
                ],
                connections: Vec::new(),
                external_resources: Vec::new(),
                sub_resources: Vec::new(),
            },
        );

        let attachment = ScriptAttachment {
            scene_path: "res://scenes/child.tscn".to_string(),
            node_name: "Inner".to_string(),
            node_full_path: "ChildRoot/Inner".to_string(),
            node_parent_path: "ParentA".to_string(),
        };
        assert!(resolve_parent_candidates_for_attachment(&graph, &attachment).is_none());
    }

    #[test]
    fn extraction_ignores_commented_out_contracts() {
        let source = r#"
# get_parent().get_node("Nope")
# get_parent().connect("missing", Callable(self, "_on_any"))
# get_parent().call("missing_method")
"#;
        let mut parser = gozen_parser::GDScriptParser::new();
        let tree = parser.parse(source).expect("script should parse");

        assert!(extract_parent_node_paths(&tree, source).is_empty());
        assert!(extract_parent_signal_names(&tree, source).is_empty());
        assert!(extract_parent_method_names(&tree, source).is_empty());
    }

    #[test]
    fn extraction_uses_code_before_inline_comment() {
        let source = r#"
func _ready():
    get_parent().get_node("RealPath") # get_parent().get_node("Nope")
    get_parent().connect("real_signal", Callable(self, "_on_any")) # get_parent().connect("fake", Callable(self, "_on_any"))
    get_parent().call("real_method") # get_parent().call("fake_method")
"#;
        let mut parser = gozen_parser::GDScriptParser::new();
        let tree = parser.parse(source).expect("script should parse");

        let nodes = extract_parent_node_paths(&tree, source);
        let signals = extract_parent_signal_names(&tree, source);
        let methods = extract_parent_method_names(&tree, source);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].value, "RealPath");
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].value, "real_signal");
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].value, "real_method");
    }
}
