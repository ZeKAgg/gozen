use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::project_settings::parse_project_godot;
use crate::tres;
use crate::tscn;

const MAX_PROJECT_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Default)]
pub struct ProjectGraph {
    pub scenes: HashMap<String, SceneData>,
    pub scripts: HashMap<String, ScriptData>,
    pub resources: HashMap<String, tres::ResourceData>,
    pub autoloads: Vec<Autoload>,
    pub input_actions: Vec<String>,
    pub class_names: HashMap<String, String>,
}

pub struct SceneData {
    pub path: String,
    pub nodes: Vec<SceneNode>,
    pub connections: Vec<SignalConnection>,
    pub external_resources: Vec<ExternalResource>,
    pub sub_resources: Vec<SceneSubResource>,
}

pub struct SceneNode {
    pub name: String,
    pub node_type: String,
    pub parent: String,
    pub full_path: String,
    pub script: Option<String>,
    pub instanced_scene: Option<String>,
    pub properties: HashMap<String, String>,
}

pub struct SignalConnection {
    pub signal: String,
    pub from_node: String,
    pub to_node: String,
    pub method: String,
}

pub struct ExternalResource {
    pub resource_type: String,
    pub path: String,
    pub id: String,
}

pub struct SceneSubResource {
    pub resource_type: String,
    pub id: String,
}

pub struct Autoload {
    pub name: String,
    pub path: String,
    pub is_singleton: bool,
}

pub struct ScriptData {
    pub path: String,
    pub class_name: Option<String>,
    pub signals: Vec<String>,
    pub functions: Vec<String>,
    pub exported_vars: Vec<ExportedVar>,
    pub attached_to_scenes: Vec<String>,
    pub attached_nodes: Vec<ScriptAttachment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptAttachment {
    pub scene_path: String,
    pub node_name: String,
    pub node_full_path: String,
    pub node_parent_path: String,
}

pub struct ExportedVar {
    pub name: String,
    pub var_type: Option<String>,
}

fn to_res_path(abs_path: &Path, project_root: &Path) -> String {
    let rel = abs_path.strip_prefix(project_root).unwrap_or(abs_path);
    format!("res://{}", rel.to_string_lossy().replace('\\', "/"))
}

fn is_godot_hidden(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    name == ".godot" || name.starts_with(".git")
}

fn read_text_file_limited(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_PROJECT_FILE_SIZE_BYTES {
        anyhow::bail!(
            "file {} is too large ({} bytes, max {} bytes)",
            path.display(),
            metadata.len(),
            MAX_PROJECT_FILE_SIZE_BYTES
        );
    }
    Ok(std::fs::read_to_string(path)?)
}

impl ProjectGraph {
    fn clear_scene_links(&mut self, scene_path: &str) {
        for script in self.scripts.values_mut() {
            script.attached_to_scenes.retain(|s| s != scene_path);
            script.attached_nodes.retain(|a| a.scene_path != scene_path);
        }
    }

    fn link_scene_scripts(&mut self, scene_path: &str) {
        let Some(scene) = self.scenes.get(scene_path) else {
            return;
        };
        let mut links: Vec<(String, ScriptAttachment)> = Vec::new();
        for node in &scene.nodes {
            if let Some(script_path) = &node.script {
                links.push((
                    script_path.clone(),
                    ScriptAttachment {
                        scene_path: scene_path.to_string(),
                        node_name: node.name.clone(),
                        node_full_path: node.full_path.clone(),
                        node_parent_path: node.parent.clone(),
                    },
                ));
            }
        }
        for (script_path, attachment) in links {
            if let Some(script) = self.scripts.get_mut(&script_path) {
                if !script.attached_to_scenes.iter().any(|s| s == scene_path) {
                    script.attached_to_scenes.push(scene_path.to_string());
                }
                if !script.attached_nodes.iter().any(|a| {
                    a.scene_path == attachment.scene_path
                        && a.node_name == attachment.node_name
                        && a.node_full_path == attachment.node_full_path
                        && a.node_parent_path == attachment.node_parent_path
                }) {
                    script.attached_nodes.push(attachment);
                }
            }
        }
    }

    pub fn build(project_root: &Path) -> Result<Self> {
        let mut graph = ProjectGraph::default();

        let project_file = project_root.join("project.godot");
        if project_file.exists() {
            let content = read_text_file_limited(&project_file)?;
            let settings = parse_project_godot(&content);
            graph.autoloads = settings
                .autoloads
                .into_iter()
                .map(|e| Autoload {
                    name: e.name,
                    path: e.path,
                    is_singleton: e.is_singleton,
                })
                .collect();
            graph.input_actions = settings.input_actions;
        }

        for entry in walkdir::WalkDir::new(project_root)
            .into_iter()
            .filter_entry(|e| !is_godot_hidden(e))
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            match path.extension().and_then(|e| e.to_str()) {
                Some("tscn") => {
                    let content = read_text_file_limited(path)?;
                    let res_path = to_res_path(path, project_root);
                    let scene = tscn::parse_tscn(&content, &res_path)?;
                    graph.scenes.insert(res_path.clone(), scene);
                }
                Some("tres") => {
                    let content = read_text_file_limited(path)?;
                    let res_path = to_res_path(path, project_root);
                    if let Ok(resource) = tres::parse_tres(&content, &res_path) {
                        graph.resources.insert(res_path, resource);
                    }
                }
                Some("gd") => {
                    let content = read_text_file_limited(path)?;
                    let res_path = to_res_path(path, project_root);
                    let script = quick_parse_script(&content, &res_path);
                    if let Some(ref name) = script.class_name {
                        graph.class_names.insert(name.clone(), res_path.clone());
                    }
                    graph.scripts.insert(res_path, script);
                }
                _ => {}
            }
        }

        let scene_paths: Vec<String> = graph.scenes.keys().cloned().collect();
        for scene_path in scene_paths {
            graph.link_scene_scripts(&scene_path);
        }

        Ok(graph)
    }

    /// Detect circular dependencies among scenes (via instanced scenes) and
    /// resources (via external_resources). Returns a list of cycle paths.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        // Build adjacency list: node -> set of nodes it depends on
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

        // Scene -> instanced scenes
        for (scene_path, scene) in &self.scenes {
            let deps: Vec<String> = scene
                .nodes
                .iter()
                .filter_map(|n| n.instanced_scene.clone())
                .collect();
            adj.entry(scene_path.clone()).or_default().extend(deps);
        }

        // Resource -> external resource dependencies
        for (res_path, resource) in &self.resources {
            let deps: Vec<String> = resource
                .external_resources
                .iter()
                .filter(|e| e.path.ends_with(".tscn") || e.path.ends_with(".tres"))
                .map(|e| e.path.clone())
                .collect();
            adj.entry(res_path.clone()).or_default().extend(deps);
        }

        let mut cycles = Vec::new();
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut path_stack: Vec<String> = Vec::new();

        for node in adj.keys() {
            if !visited.contains(node) {
                Self::dfs_cycle(
                    node,
                    &adj,
                    &mut visited,
                    &mut visiting,
                    &mut path_stack,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_cycle(
        node: &str,
        adj: &HashMap<String, Vec<String>>,
        visited: &mut std::collections::HashSet<String>,
        visiting: &mut std::collections::HashSet<String>,
        path_stack: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visiting.insert(node.to_string());
        path_stack.push(node.to_string());

        if let Some(neighbors) = adj.get(node) {
            for neighbor in neighbors {
                if visiting.contains(neighbor) {
                    // Found a cycle — extract the cycle from path_stack
                    if let Some(pos) = path_stack.iter().position(|n| n == neighbor) {
                        let mut cycle: Vec<String> = path_stack[pos..].to_vec();
                        cycle.push(neighbor.clone()); // close the cycle
                        cycles.push(cycle);
                    }
                } else if !visited.contains(neighbor) {
                    Self::dfs_cycle(neighbor, adj, visited, visiting, path_stack, cycles);
                }
            }
        }

        path_stack.pop();
        visiting.remove(node);
        visited.insert(node.to_string());
    }

    /// Incrementally update a single script file in the graph.
    pub fn update_script(&mut self, res_path: &str, content: &str) {
        let mut script = quick_parse_script(content, res_path);
        // Remove old class_name mapping
        if let Some(old) = self.scripts.get(res_path) {
            if let Some(ref old_name) = old.class_name {
                self.class_names.remove(old_name);
            }
            script.attached_to_scenes = old.attached_to_scenes.clone();
            script.attached_nodes = old.attached_nodes.clone();
        }
        if let Some(ref name) = script.class_name {
            self.class_names.insert(name.clone(), res_path.to_string());
        }
        self.scripts.insert(res_path.to_string(), script);
    }

    /// Incrementally update a single scene file in the graph.
    pub fn update_scene(&mut self, res_path: &str, content: &str) -> Result<()> {
        let scene = tscn::parse_tscn(content, res_path)?;

        // Clear old scene links from all scripts before re-adding
        self.clear_scene_links(res_path);
        self.scenes.insert(res_path.to_string(), scene);
        self.link_scene_scripts(res_path);
        Ok(())
    }

    /// Incrementally update a single resource file in the graph.
    pub fn update_resource(&mut self, res_path: &str, content: &str) {
        match tres::parse_tres(content, res_path) {
            Ok(resource) => {
                self.resources.insert(res_path.to_string(), resource);
            }
            Err(e) => {
                eprintln!("Warning: failed to parse resource {}: {}", res_path, e);
            }
        }
    }

    /// Remove a file from the graph (script, scene, or resource).
    pub fn remove_file(&mut self, res_path: &str) {
        if let Some(script) = self.scripts.remove(res_path) {
            if let Some(ref name) = script.class_name {
                self.class_names.remove(name);
            }
        }
        // If removing a scene, clean up stale attached_to_scenes references
        if self.scenes.remove(res_path).is_some() {
            self.clear_scene_links(res_path);
        }
        self.resources.remove(res_path);
    }
}

fn quick_parse_script(content: &str, path: &str) -> ScriptData {
    let mut class_name = None;
    let mut signals = Vec::new();
    let mut functions = Vec::new();
    let mut exported_vars = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if let Some(stripped) = line.strip_prefix("class_name ") {
            let rest = stripped.trim();
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or(rest)
                .trim_end_matches(':');
            class_name = Some(name.to_string());
        } else if let Some(stripped) = line.strip_prefix("signal ") {
            let rest = stripped.trim();
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or(rest)
                .trim_end_matches(':');
            signals.push(name.to_string());
        } else if line.starts_with("func ") || line.starts_with("static func ") {
            let after_func = if let Some(rest) = line.strip_prefix("func ") {
                rest
            } else {
                line.strip_prefix("static func ").unwrap_or("")
            };
            let name = after_func
                .split('(')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                functions.push(name.to_string());
            }
        } else if line.starts_with("@export") {
            // Handle all @export variants: @export var, @export_range(...) var, etc.
            // Find "var " in the line to locate the variable name
            if let Some(var_pos) = line.find("var ") {
                let after_var = line[var_pos + 4..].trim();
                let name = after_var
                    .split(|c: char| c.is_whitespace() || c == ':' || c == '=')
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    exported_vars.push(ExportedVar {
                        name: name.to_string(),
                        var_type: None,
                    });
                }
            }
        }
    }

    ScriptData {
        path: path.to_string(),
        class_name,
        signals,
        functions,
        exported_vars,
        attached_to_scenes: Vec::new(),
        attached_nodes: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_parse_script_extracts_functions() {
        let src = r#"
extends Node

func _ready() -> void:
    pass

static func build_state(x: int) -> int:
    return x
"#;
        let script = quick_parse_script(src, "res://scripts/a.gd");
        assert!(script.functions.iter().any(|f| f == "_ready"));
        assert!(script.functions.iter().any(|f| f == "build_state"));
    }

    #[test]
    fn update_scene_links_root_and_non_root_attachments() {
        let mut graph = ProjectGraph::default();
        graph.scripts.insert(
            "res://scripts/child.gd".to_string(),
            ScriptData {
                path: "res://scripts/child.gd".to_string(),
                class_name: None,
                signals: Vec::new(),
                functions: Vec::new(),
                exported_vars: Vec::new(),
                attached_to_scenes: Vec::new(),
                attached_nodes: Vec::new(),
            },
        );

        let scene = r#"[gd_scene load_steps=2 format=3]

[ext_resource type="Script" path="res://scripts/child.gd" id="1"]

[node name="Root" type="Node2D"]
script = ExtResource("1")

[node name="Child" type="Node2D" parent="Root"]
script = ExtResource("1")
"#;
        graph
            .update_scene("res://scenes/test.tscn", scene)
            .expect("scene should parse");

        let script = graph
            .scripts
            .get("res://scripts/child.gd")
            .expect("script must exist");
        assert_eq!(
            script.attached_to_scenes,
            vec!["res://scenes/test.tscn".to_string()]
        );
        assert_eq!(script.attached_nodes.len(), 2);
        assert!(script
            .attached_nodes
            .iter()
            .any(|a| a.node_full_path == "Root" && a.node_parent_path == "."));
        assert!(script
            .attached_nodes
            .iter()
            .any(|a| a.node_full_path.ends_with("/Child") && a.node_parent_path == "Root"));
    }
}
