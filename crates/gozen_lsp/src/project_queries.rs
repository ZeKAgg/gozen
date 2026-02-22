use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use gozen_project::{ProjectGraph, SceneData, SceneNode};
use tower_lsp::lsp_types::{Location, Position, Range};
use url::Url;

const MAX_LSP_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

pub enum CompletionContextKind {
    NodePath { prefix: String },
    Group { prefix: String },
    InputAction { prefix: String },
}

pub fn uri_to_res_path(uri: &Url, project_root: &Path) -> Option<String> {
    let abs = uri.to_file_path().ok()?;
    let rel = abs.strip_prefix(project_root).ok()?;
    Some(format!(
        "res://{}",
        rel.to_string_lossy().replace('\\', "/")
    ))
}

fn read_text_file_limited(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_LSP_FILE_SIZE_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

pub fn extract_node_path_at_position(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let col = position.character as usize;
    let col = col.min(line.len());

    if let Some((before, inner)) = full_string_context(line, col) {
        if is_get_node_context(&before) {
            return Some(inner);
        }
    }

    extract_dollar_path(line, col)
}

pub fn completion_context_at_position(
    source: &str,
    position: Position,
) -> Option<CompletionContextKind> {
    let line = source.lines().nth(position.line as usize)?;
    let col = position.character as usize;
    let col = col.min(line.len());
    let (before, inner) = string_context(line, col)?;

    if is_get_node_context(&before) {
        return Some(CompletionContextKind::NodePath { prefix: inner });
    }
    if is_group_context(&before) {
        return Some(CompletionContextKind::Group { prefix: inner });
    }
    if is_input_action_context(&before) {
        return Some(CompletionContextKind::InputAction { prefix: inner });
    }
    None
}

pub fn resolve_node_definition_locations(
    graph: &ProjectGraph,
    project_root: &Path,
    script_res_path: &str,
    node_path: &str,
) -> Vec<Location> {
    let Some(script) = graph.scripts.get(script_res_path) else {
        return Vec::new();
    };

    let mut locations = Vec::new();
    let mut seen = HashSet::new();

    for attachment in &script.attached_nodes {
        let Some(scene) = graph.scenes.get(&attachment.scene_path) else {
            continue;
        };
        let Some(target_full) = resolve_node_path(scene, &attachment.node_full_path, node_path)
        else {
            continue;
        };
        let Some(loc) =
            node_location_in_scene(scene, &attachment.scene_path, &target_full, project_root)
        else {
            continue;
        };
        let key = format!(
            "{}:{}:{}:{}:{}",
            loc.uri,
            loc.range.start.line,
            loc.range.start.character,
            loc.range.end.line,
            loc.range.end.character
        );
        if seen.insert(key) {
            locations.push(loc);
        }
    }

    locations.sort_by(|a, b| {
        (a.uri.as_str(), a.range.start.line, a.range.start.character).cmp(&(
            b.uri.as_str(),
            b.range.start.line,
            b.range.start.character,
        ))
    });
    locations
}

pub fn gather_signal_usage_locations(
    graph: &ProjectGraph,
    project_root: &Path,
    open_documents: &HashMap<Url, String>,
    signal: &str,
) -> Vec<Location> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for res_path in graph.scripts.keys() {
        let rel = res_path.strip_prefix("res://").unwrap_or(res_path);
        let abs = project_root.join(rel);
        let Ok(uri) = Url::from_file_path(&abs) else {
            continue;
        };
        let content = if let Some(open) = open_documents.get(&uri) {
            open.clone()
        } else if let Some(on_disk) = read_text_file_limited(&abs) {
            on_disk
        } else {
            continue;
        };
        collect_signal_usages_from_source(&uri, &content, signal, &mut out, &mut seen);
    }

    for (scene_res_path, scene) in &graph.scenes {
        for conn in &scene.connections {
            if conn.signal != signal {
                continue;
            }
            let rel = scene_res_path
                .strip_prefix("res://")
                .unwrap_or(scene_res_path);
            let abs = project_root.join(rel);
            let Ok(uri) = Url::from_file_path(&abs) else {
                continue;
            };
            let Some(content) = read_text_file_limited(&abs) else {
                continue;
            };
            for (row, line) in content.lines().enumerate() {
                if !line.trim_start().starts_with("[connection ") {
                    continue;
                }
                let needle = format!("signal=\"{}\"", signal);
                if let Some(col) = line.find(&needle) {
                    push_location(
                        &uri,
                        row as u32,
                        col as u32,
                        (col + needle.len()) as u32,
                        &mut out,
                        &mut seen,
                    );
                    break;
                }
            }
        }
        let _ = scene;
    }

    out.sort_by(|a, b| {
        (a.uri.as_str(), a.range.start.line, a.range.start.character).cmp(&(
            b.uri.as_str(),
            b.range.start.line,
            b.range.start.character,
        ))
    });
    out
}

pub fn node_path_completions_for_prefix(
    graph: &ProjectGraph,
    script_res_path: &str,
    prefix: &str,
) -> Vec<String> {
    let Some(script) = graph.scripts.get(script_res_path) else {
        return Vec::new();
    };
    let (base, partial) = split_prefix(prefix);
    let mut out = BTreeSet::new();

    for attachment in &script.attached_nodes {
        let Some(scene) = graph.scenes.get(&attachment.scene_path) else {
            continue;
        };
        let base_full = if base.is_empty() {
            Some(attachment.node_full_path.clone())
        } else {
            resolve_node_path(scene, &attachment.node_full_path, &base)
        };
        let Some(base_full) = base_full else {
            continue;
        };
        for node in &scene.nodes {
            if parent_of(&node.full_path) != Some(base_full.as_str()) {
                continue;
            }
            if !node.name.starts_with(&partial) {
                continue;
            }
            let candidate = if base.is_empty() {
                node.name.clone()
            } else {
                format!("{}/{}", base, node.name)
            };
            out.insert(candidate);
        }
    }

    out.into_iter().collect()
}

pub fn collect_project_groups(
    graph: &ProjectGraph,
    project_root: &Path,
    open_documents: &HashMap<Url, String>,
) -> Vec<String> {
    let mut out = BTreeSet::new();
    for res_path in graph.scripts.keys() {
        let rel = res_path.strip_prefix("res://").unwrap_or(res_path);
        let abs = project_root.join(rel);
        let Ok(uri) = Url::from_file_path(&abs) else {
            continue;
        };
        let content = if let Some(open) = open_documents.get(&uri) {
            open.clone()
        } else if let Some(on_disk) = read_text_file_limited(&abs) {
            on_disk
        } else {
            continue;
        };
        for line in content.lines() {
            let n = normalize_for_matching(line);
            for prefix in [
                "add_to_group(",
                "is_in_group(",
                "get_nodes_in_group(",
                "get_first_node_in_group(",
            ] {
                if let Some(name) = extract_first_string_arg_anywhere(&n, prefix) {
                    out.insert(name);
                }
            }
        }
    }
    out.into_iter().collect()
}

fn collect_signal_usages_from_source(
    uri: &Url,
    source: &str,
    signal: &str,
    out: &mut Vec<Location>,
    seen: &mut HashSet<String>,
) {
    let emit_pat = format!("{}.emit(", signal);
    let emit_signal_pat = format!("emit_signal(\"{}\"", signal);
    let connect_pat = format!(".connect(\"{}\"", signal);

    for (row, line) in source.lines().enumerate() {
        let normalized = normalize_for_matching(line);
        for pat in [&emit_pat, &emit_signal_pat, &connect_pat] {
            let mut start = 0usize;
            while let Some(idx) = normalized[start..].find(pat) {
                let col = (start + idx) as u32;
                push_location(uri, row as u32, col, col + pat.len() as u32, out, seen);
                start += idx + pat.len();
            }
        }
    }
}

fn push_location(
    uri: &Url,
    line: u32,
    start_col: u32,
    end_col: u32,
    out: &mut Vec<Location>,
    seen: &mut HashSet<String>,
) {
    let key = format!("{}:{}:{}:{}", uri, line, start_col, end_col);
    if !seen.insert(key) {
        return;
    }
    out.push(Location {
        uri: uri.clone(),
        range: Range {
            start: Position {
                line,
                character: start_col,
            },
            end: Position {
                line,
                character: end_col,
            },
        },
    });
}

fn node_location_in_scene(
    scene: &SceneData,
    scene_res_path: &str,
    target_full: &str,
    project_root: &Path,
) -> Option<Location> {
    let target = scene.nodes.iter().find(|n| n.full_path == target_full)?;
    let rel = scene_res_path
        .strip_prefix("res://")
        .unwrap_or(scene_res_path);
    let abs = project_root.join(rel);
    let uri = Url::from_file_path(&abs).ok()?;
    let content = read_text_file_limited(&abs)?;

    for (row, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("[node ") {
            continue;
        }
        let name = parse_attr(trimmed, "name").unwrap_or_default();
        if name != target.name {
            continue;
        }
        let parent = parse_attr(trimmed, "parent").unwrap_or_else(|| ".".to_string());
        if parent != target.parent {
            continue;
        }
        return Some(Location {
            uri,
            range: Range {
                start: Position {
                    line: row as u32,
                    character: 0,
                },
                end: Position {
                    line: row as u32,
                    character: line.len() as u32,
                },
            },
        });
    }
    None
}

fn parse_attr(line: &str, key: &str) -> Option<String> {
    let search = format!("{}=\"", key);
    let start = line.find(&search)?;
    let value_start = start + search.len();
    let value_end = line[value_start..].find('"')? + value_start;
    Some(line[value_start..value_end].to_string())
}

fn split_prefix(prefix: &str) -> (String, String) {
    let p = prefix.trim();
    if p.is_empty() {
        return (String::new(), String::new());
    }
    if p.ends_with('/') {
        return (p.trim_end_matches('/').to_string(), String::new());
    }
    if let Some((base, partial)) = p.rsplit_once('/') {
        (base.to_string(), partial.to_string())
    } else {
        (String::new(), p.to_string())
    }
}

fn resolve_node_path(scene: &SceneData, start_full: &str, path: &str) -> Option<String> {
    let normalized = path.trim();
    if normalized.is_empty() {
        return Some(start_full.to_string());
    }
    if normalized.starts_with('/')
        || normalized.starts_with('%')
        || normalized.contains(':')
        || normalized.contains("//")
    {
        return None;
    }

    let mut current = start_full.to_string();
    for seg in normalized.split('/') {
        if seg.is_empty() {
            return None;
        }
        if seg == "." {
            continue;
        }
        if seg == ".." {
            current = parent_of(&current)?.to_string();
            continue;
        }
        let matches: Vec<&SceneNode> = scene
            .nodes
            .iter()
            .filter(|n| parent_of(&n.full_path) == Some(current.as_str()) && n.name == seg)
            .collect();
        if matches.len() != 1 {
            return None;
        }
        current = matches[0].full_path.clone();
    }
    Some(current)
}

fn parent_of(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(p, _)| p)
}

fn extract_dollar_path(line: &str, col: usize) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = col.min(bytes.len());
    while i > 0 {
        let b = bytes[i - 1];
        if b == b'$' {
            let start = i;
            let mut end = start;
            while end < bytes.len() {
                let c = bytes[end] as char;
                if c.is_ascii_alphanumeric() || c == '_' || c == '/' || c == '.' {
                    end += 1;
                } else {
                    break;
                }
            }
            if col >= start && col <= end && end > start {
                return Some(line[start..end].to_string());
            }
            return None;
        }
        if b.is_ascii_whitespace() || b == b'(' || b == b')' || b == b';' || b == b',' {
            break;
        }
        i -= 1;
    }
    None
}

fn string_context(line: &str, col: usize) -> Option<(String, String)> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut quote = b'"';
    let mut start = 0usize;
    let mut i = 0usize;
    while i < col.min(bytes.len()) {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == quote {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' {
            in_string = true;
            quote = b;
            start = i;
        }
        i += 1;
    }
    if !in_string || start >= col || col > line.len() {
        return None;
    }
    let before = line[..start].to_string();
    let inner = line[start + 1..col].to_string();
    Some((before, inner))
}

fn full_string_context(line: &str, col: usize) -> Option<(String, String)> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut quote = b'"';
    let mut start = 0usize;
    let mut i = 0usize;
    while i < col.min(bytes.len()) {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == quote {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' || b == b'\'' {
            in_string = true;
            quote = b;
            start = i;
        }
        i += 1;
    }
    if !in_string || start >= col || col > line.len() {
        return None;
    }
    let mut end = col.min(bytes.len());
    while end < bytes.len() {
        let b = bytes[end];
        if b == b'\\' {
            end += 2;
            continue;
        }
        if b == quote {
            let before = line[..start].to_string();
            let inner = line[start + 1..end].to_string();
            return Some((before, inner));
        }
        end += 1;
    }
    None
}

fn is_get_node_context(before: &str) -> bool {
    let n: String = before.chars().filter(|c| !c.is_whitespace()).collect();
    n.ends_with("get_node(") || n.ends_with("get_node_or_null(")
}

fn is_group_context(before: &str) -> bool {
    let n: String = before.chars().filter(|c| !c.is_whitespace()).collect();
    n.ends_with("add_to_group(")
        || n.ends_with("is_in_group(")
        || n.ends_with("get_nodes_in_group(")
        || n.ends_with("get_first_node_in_group(")
}

fn is_input_action_context(before: &str) -> bool {
    let n: String = before.chars().filter(|c| !c.is_whitespace()).collect();
    n.contains("Input.is_action_")
        || n.contains("Input.get_action_")
        || n.contains("InputMap.has_action(")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_node_path_contexts() {
        let src = "func _ready():\n\tvar n = get_node(\"UI/H\")\n";
        let col = src
            .lines()
            .nth(1)
            .and_then(|l| l.find("UI/H"))
            .map(|i| i + 3)
            .unwrap();
        let c = completion_context_at_position(
            src,
            Position {
                line: 1,
                character: col as u32,
            },
        );
        assert!(matches!(c, Some(CompletionContextKind::NodePath { .. })));
    }

    #[test]
    fn detects_group_contexts() {
        let src = "func _ready():\n\tadd_to_group(\"enem\")\n";
        let col = src
            .lines()
            .nth(1)
            .and_then(|l| l.find("enem"))
            .map(|i| i + 3)
            .unwrap();
        let c = completion_context_at_position(
            src,
            Position {
                line: 1,
                character: col as u32,
            },
        );
        assert!(matches!(c, Some(CompletionContextKind::Group { .. })));
    }

    #[test]
    fn detects_input_action_contexts() {
        let src = "func _process(_d):\n\tif Input.is_action_pressed(\"ui_\"):\n\t\tpass\n";
        let col = src
            .lines()
            .nth(1)
            .and_then(|l| l.find("ui_"))
            .map(|i| i + 2)
            .unwrap();
        let c = completion_context_at_position(
            src,
            Position {
                line: 1,
                character: col as u32,
            },
        );
        assert!(matches!(c, Some(CompletionContextKind::InputAction { .. })));
    }
}
