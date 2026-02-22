// Parse project.godot (INI-like): [autoload], [input]

pub struct ProjectSettings {
    pub autoloads: Vec<AutoloadEntry>,
    pub input_actions: Vec<String>,
}

pub struct AutoloadEntry {
    pub name: String,
    pub path: String,
    pub is_singleton: bool,
}

pub fn parse_project_godot(content: &str) -> ProjectSettings {
    let mut current_section = String::new();
    let mut autoloads = Vec::new();
    let mut input_actions = Vec::new();

    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
        } else if let Some((key, raw_value)) = line.split_once('=') {
            let key = key.trim();
            // Handle multi-line values: if value contains unclosed braces/brackets,
            // keep reading until they close. This handles input action definitions
            // that span multiple lines.
            let mut value = raw_value.trim().to_string();
            let open_braces = value.chars().filter(|c| *c == '{').count();
            let close_braces = value.chars().filter(|c| *c == '}').count();
            if open_braces > close_braces {
                while let Some(cont) = lines.peek() {
                    let cont = cont.trim();
                    value.push_str(cont);
                    lines.next();
                    let open = value.chars().filter(|c| *c == '{').count();
                    let close = value.chars().filter(|c| *c == '}').count();
                    if open <= close {
                        break;
                    }
                }
            }

            match current_section.as_str() {
                "autoload" => {
                    let is_singleton = value.starts_with('*') || value.starts_with("\"*");
                    let path = value.trim_matches('"').trim_start_matches('*').to_string();
                    autoloads.push(AutoloadEntry {
                        name: key.to_string(),
                        path,
                        is_singleton,
                    });
                }
                "input" => {
                    input_actions.push(key.to_string());
                }
                _ => {}
            }
        }
    }

    ProjectSettings {
        autoloads,
        input_actions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_autoloads() {
        let content = r#"
[autoload]
Global="*res://scripts/global.gd"
Utils="res://scripts/utils.gd"
"#;
        let settings = parse_project_godot(content);
        assert_eq!(settings.autoloads.len(), 2);
        assert_eq!(settings.autoloads[0].name, "Global");
        assert!(settings.autoloads[0].is_singleton);
        assert_eq!(settings.autoloads[0].path, "res://scripts/global.gd");
        assert_eq!(settings.autoloads[1].name, "Utils");
        assert!(!settings.autoloads[1].is_singleton);
    }

    #[test]
    fn test_parse_input_actions() {
        let content = r#"
[input]
move_left={"deadzone": 0.5, "events": []}
move_right={"deadzone": 0.5, "events": []}
"#;
        let settings = parse_project_godot(content);
        assert_eq!(settings.input_actions.len(), 2);
        assert!(settings.input_actions.contains(&"move_left".to_string()));
        assert!(settings.input_actions.contains(&"move_right".to_string()));
    }

    #[test]
    fn test_parse_empty_content() {
        let settings = parse_project_godot("");
        assert!(settings.autoloads.is_empty());
        assert!(settings.input_actions.is_empty());
    }

    #[test]
    fn test_ignores_other_sections() {
        let content = r#"
[application]
config/name="My Game"

[autoload]
Global="*res://scripts/global.gd"

[rendering]
textures/vram_compression/import_etc2_astc=true
"#;
        let settings = parse_project_godot(content);
        assert_eq!(settings.autoloads.len(), 1);
        assert_eq!(settings.autoloads[0].name, "Global");
    }
}
