// Parse .tscn (Godot scene format)

use anyhow::{Context, Result};

use crate::graph::{ExternalResource, SceneData, SceneNode, SceneSubResource, SignalConnection};

fn parse_attr(line: &str, key: &str) -> Option<String> {
    let search = format!("{}=\"", key);
    let start = line.find(&search)?;
    let value_start = start + search.len();
    let value_end = line[value_start..].find('"')? + value_start;
    Some(line[value_start..value_end].to_string())
}

pub fn parse_tscn(content: &str, path: &str) -> Result<SceneData> {
    let mut scene = SceneData {
        path: path.to_string(),
        nodes: Vec::new(),
        connections: Vec::new(),
        external_resources: Vec::new(),
        sub_resources: Vec::new(),
    };

    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.starts_with("[ext_resource") {
            let ext = parse_ext_resource(line)
                .with_context(|| format!("parse ext_resource: {}", line))?;
            scene.external_resources.push(ext);
        } else if line.starts_with("[sub_resource") {
            let sub = parse_sub_resource(line)
                .with_context(|| format!("parse sub_resource: {}", line))?;
            scene.sub_resources.push(sub);
            let _ = parse_node_properties(&mut lines);
        } else if line.starts_with("[node ") {
            let mut n = parse_node_line(line)?;
            n.properties = parse_node_properties(&mut lines);
            if let Some(script_val) = n.properties.get("script") {
                if let Some(path) = resolve_ext_resource(script_val, &scene.external_resources) {
                    n.script = Some(path);
                }
            }
            // Resolve instanced scenes: instance=ExtResource("N") on the [node] line
            if let Some(instance_val) = parse_instance_attr(line) {
                if let Some(path) = resolve_ext_resource(&instance_val, &scene.external_resources) {
                    n.instanced_scene = Some(path);
                }
            }
            scene.nodes.push(n);
        } else if line.starts_with("[connection ") {
            let conn =
                parse_connection(line).with_context(|| format!("parse connection: {}", line))?;
            scene.connections.push(conn);
        }
    }

    resolve_full_paths(&mut scene.nodes);
    Ok(scene)
}

fn parse_sub_resource(line: &str) -> Result<SceneSubResource> {
    let resource_type = parse_attr(line, "type").unwrap_or_else(|| "Resource".to_string());
    let id = parse_attr(line, "id").unwrap_or_default();
    Ok(SceneSubResource { resource_type, id })
}

fn parse_ext_resource(line: &str) -> Result<ExternalResource> {
    let resource_type = parse_attr(line, "type").unwrap_or_else(|| "Resource".to_string());
    let path = parse_attr(line, "path").unwrap_or_default();
    let id = parse_attr(line, "id").unwrap_or_default();
    Ok(ExternalResource {
        resource_type,
        path,
        id,
    })
}

use std::collections::HashMap;
use std::iter::Peekable;

fn parse_node_properties<'a, I>(lines: &mut Peekable<I>) -> HashMap<String, String>
where
    I: Iterator<Item = &'a str>,
{
    let mut properties = HashMap::new();
    loop {
        match lines.peek() {
            None => break,
            Some(line) => {
                let line = line.trim();
                if line.is_empty() {
                    lines.next();
                    continue;
                }
                if line.starts_with('[') {
                    // Don't consume the section header -- leave it for the caller
                    break;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim().to_string();
                    let v = v.trim().trim_matches('"').to_string();
                    properties.insert(k, v);
                }
                lines.next();
            }
        }
    }
    properties
}

fn parse_node_line(line: &str) -> Result<SceneNode> {
    let name = parse_attr(line, "name").unwrap_or_else(|| "Node".to_string());
    let node_type = parse_attr(line, "type").unwrap_or_else(|| "Node".to_string());
    let parent = parse_attr(line, "parent").unwrap_or_else(|| ".".to_string());
    Ok(SceneNode {
        name: name.clone(),
        node_type,
        parent: parent.clone(),
        full_path: name,
        script: None,
        instanced_scene: None,
        properties: HashMap::new(),
    })
}

fn parse_connection(line: &str) -> Result<SignalConnection> {
    let signal = parse_attr(line, "signal").unwrap_or_default();
    let from_node = parse_attr(line, "from").unwrap_or_else(|| ".".to_string());
    let to_node = parse_attr(line, "to").unwrap_or_else(|| ".".to_string());
    let method = parse_attr(line, "method").unwrap_or_default();
    Ok(SignalConnection {
        signal,
        from_node,
        to_node,
        method,
    })
}

/// Extract the instance=ExtResource("...") value from a [node] line.
/// The attribute format is: instance=ExtResource("id") (no quotes around key).
fn parse_instance_attr(line: &str) -> Option<String> {
    // Look for instance=ExtResource( in the line
    let search = "instance=ExtResource(";
    let start = line.find(search)?;
    let value_start = start + "instance=".len();
    // Find the closing parenthesis
    let rest = &line[value_start..];
    let end = rest.find(')')?;
    // Return the full ExtResource(...) string
    Some(rest[..=end].to_string())
}

fn resolve_ext_resource(value: &str, ext: &[ExternalResource]) -> Option<String> {
    let value = value.trim();
    if !value.starts_with("ExtResource(") {
        return None;
    }
    let inner = value
        .trim_start_matches("ExtResource(")
        .trim_end_matches(')');
    let id = inner.trim_matches('"').trim();
    ext.iter().find(|e| e.id == id).map(|e| e.path.clone())
}

fn resolve_full_paths(nodes: &mut [SceneNode]) {
    let root_name = nodes
        .iter()
        .find(|n| n.parent == "." || n.parent.is_empty())
        .map(|n| n.name.clone())
        .unwrap_or_else(|| ".".to_string());

    // Build a name -> full_path lookup that we update as we resolve.
    // This replaces the O(n^2) multi-pass approach with O(n) iterative resolution.
    let mut resolved: HashMap<String, String> = HashMap::with_capacity(nodes.len());

    // First pass: resolve root-level nodes (parent is "." or empty)
    for node in nodes.iter_mut() {
        if node.parent == "." || node.parent.is_empty() {
            if node.name == root_name {
                node.full_path = node.name.clone();
            } else {
                node.full_path = format!("{}/{}", &root_name, node.name);
            }
            resolved.insert(node.name.clone(), node.full_path.clone());
        }
    }

    // Second pass: resolve children using the lookup map.
    // Iterate until no more progress (handles out-of-order nodes).
    let max_passes = nodes.len();
    for _ in 0..max_passes {
        let mut resolved_any = false;
        for node in nodes.iter_mut() {
            if node.parent == "." || node.parent.is_empty() {
                continue; // Already resolved
            }
            if node.full_path != node.name {
                continue; // Already resolved in a prior pass
            }
            if let Some(parent_full) = resolve_parent_ref(&node.parent, &root_name, &resolved) {
                node.full_path = format!("{}/{}", parent_full, node.name);
                resolved.insert(node.name.clone(), node.full_path.clone());
                resolved_any = true;
            }
        }
        if !resolved_any {
            break;
        }
    }

    // Final fallback: any still-unresolved nodes get best-effort paths
    for node in nodes.iter_mut() {
        if node.parent != "." && !node.parent.is_empty() && node.full_path == node.name {
            if let Some(parent_full) = resolve_parent_ref(&node.parent, &root_name, &resolved) {
                node.full_path = format!("{}/{}", parent_full, node.name);
            } else {
                node.full_path = format!("{}/{}", node.parent, node.name);
            }
        }
    }
}

fn resolve_parent_ref(
    parent: &str,
    root_name: &str,
    resolved: &HashMap<String, String>,
) -> Option<String> {
    if parent == "." || parent.is_empty() {
        return Some(root_name.to_string());
    }
    if let Some(parent_full) = resolved.get(parent) {
        return Some(parent_full.clone());
    }
    if parent.contains('/') {
        let p = parent.trim_start_matches("./").trim_start_matches('/');
        if p.is_empty() {
            return None;
        }
        if p == root_name || p.starts_with(&format!("{}/", root_name)) {
            return Some(p.to_string());
        }
        return Some(format!("{}/{}", root_name, p));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_tscn() {
        let content = r#"[gd_scene load_steps=2 format=3]

[ext_resource type="Script" path="res://scripts/player.gd" id="1"]

[node name="Player" type="CharacterBody2D"]
script = ExtResource("1")

[node name="Sprite" type="Sprite2D" parent="."]

[connection signal="ready" from="." to="." method="_on_ready"]
"#;
        let scene = parse_tscn(content, "res://scenes/player.tscn").unwrap();
        assert_eq!(scene.external_resources.len(), 1);
        assert_eq!(scene.external_resources[0].resource_type, "Script");
        assert_eq!(scene.external_resources[0].path, "res://scripts/player.gd");
        assert_eq!(scene.nodes.len(), 2);
        assert_eq!(scene.nodes[0].name, "Player");
        assert_eq!(scene.nodes[1].name, "Sprite");
        assert_eq!(scene.nodes[1].parent, ".");
        assert_eq!(scene.connections.len(), 1);
        assert_eq!(scene.connections[0].signal, "ready");
    }

    #[test]
    fn test_resolve_full_paths() {
        let mut nodes = vec![
            SceneNode {
                name: "Root".to_string(),
                node_type: "Node".to_string(),
                parent: ".".to_string(),
                full_path: "Root".to_string(),
                script: None,
                instanced_scene: None,
                properties: HashMap::new(),
            },
            SceneNode {
                name: "Child".to_string(),
                node_type: "Node".to_string(),
                parent: "Root".to_string(),
                full_path: "Child".to_string(),
                script: None,
                instanced_scene: None,
                properties: HashMap::new(),
            },
        ];
        resolve_full_paths(&mut nodes);
        assert_eq!(nodes[0].full_path, "Root");
        assert_eq!(nodes[1].full_path, "Root/Child");
    }

    #[test]
    fn test_parse_empty_scene() {
        let content = "[gd_scene format=3]\n";
        let scene = parse_tscn(content, "res://empty.tscn").unwrap();
        assert!(scene.nodes.is_empty());
        assert!(scene.connections.is_empty());
    }

    #[test]
    fn test_parse_sub_resource_sections() {
        let content = r#"[gd_scene load_steps=2 format=3]

[sub_resource type="ShaderMaterial" id="ShaderMaterial_1"]
shader_parameter/color = Color(1, 0, 0, 1)

[node name="Root" type="Node2D"]
material = SubResource("ShaderMaterial_1")
"#;
        let scene = parse_tscn(content, "res://scenes/with_sub.tscn").unwrap();
        assert_eq!(scene.sub_resources.len(), 1);
        assert_eq!(scene.sub_resources[0].resource_type, "ShaderMaterial");
        assert_eq!(scene.sub_resources[0].id, "ShaderMaterial_1");
        assert_eq!(
            scene.nodes[0]
                .properties
                .get("material")
                .map(|s| s.as_str()),
            Some("SubResource(\"ShaderMaterial_1\")")
        );
    }

    #[test]
    fn test_resolve_parent_path_with_slash_under_root() {
        let content = r#"[gd_scene load_steps=1 format=3]

[node name="Host" type="Node2D"]
[node name="Anchor" type="Node2D" parent="."]
[node name="Child" type="Node2D" parent="Anchor"]
[node name="Leaf" type="Node2D" parent="Anchor/Child"]
"#;
        let scene = parse_tscn(content, "res://scenes/host.tscn").unwrap();
        let leaf = scene.nodes.iter().find(|n| n.name == "Leaf").unwrap();
        assert_eq!(leaf.full_path, "Host/Anchor/Child/Leaf");
    }

    #[test]
    fn test_resolve_parent_path_prefix_collision_with_root_name() {
        let content = r#"[gd_scene load_steps=1 format=3]

[node name="Root" type="Node2D"]
[node name="Rooted" type="Node2D" parent="."]
[node name="Branch" type="Node2D" parent="Rooted"]
[node name="Leaf" type="Node2D" parent="Rooted/Branch"]
"#;
        let scene = parse_tscn(content, "res://scenes/root_collision.tscn").unwrap();
        let leaf = scene.nodes.iter().find(|n| n.name == "Leaf").unwrap();
        assert_eq!(leaf.full_path, "Root/Rooted/Branch/Leaf");
    }
}
