use crate::GozenConfig;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Strip JSONC comments (// and /* */) from content, respecting string boundaries.
/// Uses &str slicing to preserve multi-byte UTF-8 characters correctly.
fn strip_jsonc_comments(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let bytes = content.as_bytes();
    let n = bytes.len();
    let mut i = 0;

    while i < n {
        let b = bytes[i];

        // Inside a double-quoted string — copy verbatim (preserves UTF-8)
        if b == b'"' {
            let start = i;
            i += 1;
            while i < n {
                if bytes[i] == b'\\' && i + 1 < n {
                    i += 2; // skip escaped character
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push_str(&content[start..i]);
            continue;
        }

        // Line comment — skip to end of line
        if b == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
            i += 2;
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            if i < n {
                out.push('\n');
                i += 1;
            }
            continue;
        }

        // Block comment — skip to */
        if b == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < n {
                i += 2;
            }
            continue;
        }

        // Regular content — find the next special character and copy the span
        let start = i;
        i += 1;
        while i < n && bytes[i] != b'"' && bytes[i] != b'/' {
            i += 1;
        }
        out.push_str(&content[start..i]);
    }

    out
}

/// Walk up from start_dir to find gozen.json or gozen.jsonc.
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let json = dir.join("gozen.json");
        if json.exists() {
            return Some(json);
        }
        let jsonc = dir.join("gozen.jsonc");
        if jsonc.exists() {
            return Some(jsonc);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Maximum config file size (1 MB) to prevent DoS via extremely large files.
const MAX_CONFIG_SIZE: u64 = 1_024 * 1_024;

/// Load config from a specific file path. Supports .jsonc (strips // and /* */ comments).
pub fn load_config_from_path(path: &Path) -> Result<GozenConfig> {
    // Check file size before reading to prevent DoS
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_CONFIG_SIZE {
        anyhow::bail!(
            "Config file {} is too large ({} bytes, max {} bytes)",
            path.display(),
            metadata.len(),
            MAX_CONFIG_SIZE
        );
    }
    let content = std::fs::read_to_string(path)?;
    let content = if path.extension().is_some_and(|e| e == "jsonc") {
        strip_jsonc_comments(&content)
    } else {
        content
    };
    let mut config: GozenConfig = serde_json::from_str(&content)?;
    validate_config(&mut config);
    Ok(config)
}

/// Validate and clamp config values to reasonable bounds.
fn validate_config(config: &mut GozenConfig) {
    // Clamp indent_width to [1, 16] to prevent absurd indentation
    if config.formatter.indent_width == 0 {
        config.formatter.indent_width = 4;
    } else if config.formatter.indent_width > 16 {
        config.formatter.indent_width = 16;
    }

    // Clamp line_width to [40, 500] to prevent degenerate formatting
    config.formatter.line_width = config.formatter.line_width.clamp(40, 500);

    // Validate indent_style
    if config.formatter.indent_style != "tab" && config.formatter.indent_style != "space" {
        config.formatter.indent_style = "tab".to_string();
    }

    // Validate end_of_line
    if !matches!(config.formatter.end_of_line.as_str(), "lf" | "crlf" | "cr") {
        config.formatter.end_of_line = "lf".to_string();
    }
}

/// Load config from the given directory (walks up to find gozen.json). Returns default if no file found.
/// Supports gozen.jsonc: strips // and /* */ comments before parsing.
pub fn load_config(start_dir: &Path) -> Result<GozenConfig> {
    let config_path = find_config(start_dir);
    match config_path {
        Some(path) => load_config_from_path(&path),
        None => Ok(GozenConfig::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_config_no_file_returns_defaults() {
        let dir = std::env::temp_dir().join("gozen_config_test_noconfig");
        let _ = fs::create_dir_all(&dir);
        let config = load_config(&dir).unwrap();
        assert_eq!(config.formatter.line_width, 100);
        assert_eq!(config.formatter.indent_style, "tab");
        assert!(config.linter.enabled);
        assert_eq!(config.files.includes, vec!["**/*.gd", "**/*.gdshader"]);
    }

    #[test]
    fn test_load_config_with_file_applies_overrides() {
        let dir = std::env::temp_dir().join("gozen_config_test_withfile");
        let _ = fs::create_dir_all(&dir);
        let config_path = dir.join("gozen.json");
        fs::write(
            &config_path,
            r#"{"formatter":{"lineWidth":80,"indentStyle":"space"}}"#,
        )
        .unwrap();
        let config = load_config(&dir).unwrap();
        assert_eq!(config.formatter.line_width, 80);
        assert_eq!(config.formatter.indent_style, "space");
        fs::remove_file(config_path).ok();
    }

    #[test]
    fn test_find_config_finds_in_start_dir() {
        let dir = std::env::temp_dir().join("gozen_find_test");
        let _ = fs::create_dir_all(&dir);
        let config_path = dir.join("gozen.json");
        fs::write(&config_path, "{}").unwrap();
        let found = find_config(&dir);
        assert_eq!(found.as_deref(), Some(config_path.as_path()));
        fs::remove_file(config_path).ok();
    }

    #[test]
    fn test_load_config_jsonc_strips_comments() {
        let dir = std::env::temp_dir().join("gozen_config_test_jsonc");
        let _ = fs::create_dir_all(&dir);
        let config_path = dir.join("gozen.jsonc");
        fs::write(
            &config_path,
            r#"{
            // line comment
            "formatter": { "lineWidth": 80 /* block */, "indentStyle": "space" }
            }"#,
        )
        .unwrap();
        let config = load_config_from_path(&config_path).unwrap();
        assert_eq!(config.formatter.line_width, 80);
        assert_eq!(config.formatter.indent_style, "space");
        fs::remove_file(config_path).ok();
    }
}
