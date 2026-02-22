// Parse .tres (Godot text resource format)

use std::collections::HashMap;

use anyhow::Result;

use crate::graph::ExternalResource;

/// Parsed representation of a .tres text resource file.
pub struct ResourceData {
    pub path: String,
    pub resource_type: Option<String>,
    pub external_resources: Vec<ExternalResource>,
    pub sub_resources: Vec<SubResource>,
    pub properties: HashMap<String, String>,
}

pub struct SubResource {
    pub resource_type: String,
    pub id: String,
    pub properties: HashMap<String, String>,
}

/// Parse a .tres file content into a ResourceData struct.
pub fn parse_tres(content: &str, path: &str) -> Result<ResourceData> {
    let mut resource = ResourceData {
        path: path.to_string(),
        resource_type: None,
        external_resources: Vec::new(),
        sub_resources: Vec::new(),
        properties: HashMap::new(),
    };

    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();

        if line.starts_with("[gd_resource") {
            // Header: [gd_resource type="Theme" ...]
            resource.resource_type = parse_attr(line, "type");
        } else if line.starts_with("[ext_resource") {
            let ext = parse_ext_resource(line);
            resource.external_resources.push(ext);
        } else if line.starts_with("[sub_resource") {
            let res_type = parse_attr(line, "type").unwrap_or_else(|| "Resource".to_string());
            let id = parse_attr(line, "id").unwrap_or_default();
            let props = parse_section_properties(&mut lines);
            resource.sub_resources.push(SubResource {
                resource_type: res_type,
                id,
                properties: props,
            });
        } else if line.starts_with("[resource]") {
            // The main resource's properties
            resource.properties = parse_section_properties(&mut lines);
        }
    }

    Ok(resource)
}

/// Parse a key="value" attribute from a section header line.
fn parse_attr(line: &str, key: &str) -> Option<String> {
    let search = format!("{}=\"", key);
    let start = line.find(&search)?;
    let value_start = start + search.len();
    let value_end = line[value_start..].find('"')? + value_start;
    Some(line[value_start..value_end].to_string())
}

/// Parse an [ext_resource] line.
fn parse_ext_resource(line: &str) -> ExternalResource {
    let resource_type = parse_attr(line, "type").unwrap_or_else(|| "Resource".to_string());
    let path = parse_attr(line, "path").unwrap_or_default();
    let id = parse_attr(line, "id").unwrap_or_default();
    ExternalResource {
        resource_type,
        path,
        id,
    }
}

/// Parse key=value properties until the next section header or end of input.
fn parse_section_properties<'a, I>(lines: &mut std::iter::Peekable<I>) -> HashMap<String, String>
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
                    break; // Don't consume the next section header
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_tres() {
        let content = r#"[gd_resource type="Theme" format=3]

[ext_resource type="Font" path="res://fonts/main.tres" id="1"]

[resource]
default_font = ExtResource("1")
"#;
        let resource = parse_tres(content, "res://theme.tres").unwrap();
        assert_eq!(resource.resource_type, Some("Theme".to_string()));
        assert_eq!(resource.external_resources.len(), 1);
        assert_eq!(resource.external_resources[0].resource_type, "Font");
        assert_eq!(
            resource.properties.get("default_font").unwrap(),
            "ExtResource(\"1\")"
        );
    }

    #[test]
    fn test_parse_empty_tres() {
        let content = "[gd_resource type=\"Resource\" format=3]\n";
        let resource = parse_tres(content, "res://empty.tres").unwrap();
        assert_eq!(resource.resource_type, Some("Resource".to_string()));
        assert!(resource.external_resources.is_empty());
        assert!(resource.sub_resources.is_empty());
    }

    #[test]
    fn test_parse_sub_resources() {
        let content = r#"[gd_resource type="StyleBoxFlat" format=3]

[sub_resource type="StyleBoxFlat" id="1"]
bg_color = Color(0.2, 0.2, 0.2, 1)

[resource]
normal = SubResource("1")
"#;
        let resource = parse_tres(content, "res://style.tres").unwrap();
        assert_eq!(resource.sub_resources.len(), 1);
        assert_eq!(resource.sub_resources[0].resource_type, "StyleBoxFlat");
    }
}
