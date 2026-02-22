use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Migrate from gdtoolkit's .gdlintrc to gozen.json.
pub fn run(from: &str, start_dir: &Path) -> anyhow::Result<()> {
    if from != "gdlintrc" {
        anyhow::bail!(
            "Unknown migration source: \"{}\". Supported: gdlintrc",
            from
        );
    }

    // Look for .gdlintrc in the project directory
    let gdlintrc_path = find_gdlintrc(start_dir);
    let gdlintrc_path = match gdlintrc_path {
        Some(p) => p,
        None => {
            anyhow::bail!(
                "No .gdlintrc file found in {} or parent directories.",
                start_dir.display()
            );
        }
    };

    println!("Found: {}", gdlintrc_path.display());

    let content = std::fs::read_to_string(&gdlintrc_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", gdlintrc_path.display(), e))?;
    let config = parse_gdlintrc(&content);

    // Build gozen.json structure
    let mut gozen = serde_json::Map::new();

    // Formatter settings
    let mut formatter = serde_json::Map::new();
    formatter.insert("enabled".into(), serde_json::Value::Bool(true));

    if let Some(max_line) = config.get("max-line-length") {
        if let Ok(n) = max_line.parse::<u64>() {
            formatter.insert("lineWidth".into(), serde_json::Value::Number(n.into()));
            println!(
                "  Migrated: max-line-length={} -> formatter.lineWidth={}",
                max_line, n
            );
        }
    }

    if let Some(tab) = config.get("tab-characters") {
        match tab.as_str() {
            "true" | "True" | "1" => {
                formatter.insert(
                    "indentStyle".into(),
                    serde_json::Value::String("tab".into()),
                );
                println!("  Migrated: tab-characters=true -> formatter.indentStyle=tab");
            }
            "false" | "False" | "0" => {
                formatter.insert(
                    "indentStyle".into(),
                    serde_json::Value::String("space".into()),
                );
                println!("  Migrated: tab-characters=false -> formatter.indentStyle=space");
            }
            _ => {}
        }
    }

    gozen.insert("formatter".into(), serde_json::Value::Object(formatter));

    // Linter rules
    let mut linter = serde_json::Map::new();
    linter.insert("enabled".into(), serde_json::Value::Bool(true));

    let mut rules = serde_json::Map::new();
    rules.insert("recommended".into(), serde_json::Value::Bool(true));

    // Map disabled gdlint rules to gozen rules
    let mut _disabled_count = 0;
    if let Some(disabled) = config.get("disabled") {
        let disabled_rules: Vec<&str> = disabled
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut style_rules = serde_json::Map::new();
        let mut correctness_rules = serde_json::Map::new();
        let mut suspicious_rules = serde_json::Map::new();

        for rule in &disabled_rules {
            if let Some((group, gozen_rule)) = map_gdlint_rule(rule) {
                match group {
                    "style" => {
                        style_rules
                            .insert(gozen_rule.into(), serde_json::Value::String("off".into()));
                    }
                    "correctness" => {
                        correctness_rules
                            .insert(gozen_rule.into(), serde_json::Value::String("off".into()));
                    }
                    "suspicious" => {
                        suspicious_rules
                            .insert(gozen_rule.into(), serde_json::Value::String("off".into()));
                    }
                    _ => {}
                }
                println!(
                    "  Migrated: disabled {} -> {}/{}: off",
                    rule, group, gozen_rule
                );
                _disabled_count += 1;
            } else {
                println!("  Skipped:  disabled {} (no gozen equivalent)", rule);
            }
        }

        if !style_rules.is_empty() {
            rules.insert("style".into(), serde_json::Value::Object(style_rules));
        }
        if !correctness_rules.is_empty() {
            rules.insert(
                "correctness".into(),
                serde_json::Value::Object(correctness_rules),
            );
        }
        if !suspicious_rules.is_empty() {
            rules.insert(
                "suspicious".into(),
                serde_json::Value::Object(suspicious_rules),
            );
        }
    }

    linter.insert("rules".into(), serde_json::Value::Object(rules));
    gozen.insert("linter".into(), serde_json::Value::Object(linter));

    // Write gozen.json
    let output_path = start_dir.join("gozen.json");
    if output_path.exists() {
        println!("\nWarning: gozen.json already exists. Writing to gozen.migrated.json instead.");
        let alt_path = start_dir.join("gozen.migrated.json");
        if let Ok(meta) = std::fs::symlink_metadata(&alt_path) {
            if meta.file_type().is_symlink() {
                anyhow::bail!("Refusing to overwrite symlinked path: {}", alt_path.display());
            }
        }
        let json = serde_json::to_string_pretty(&serde_json::Value::Object(gozen))?;
        std::fs::write(&alt_path, json)?;
        println!("Wrote: {}", alt_path.display());
    } else {
        if let Ok(meta) = std::fs::symlink_metadata(&output_path) {
            if meta.file_type().is_symlink() {
                anyhow::bail!(
                    "Refusing to overwrite symlinked path: {}",
                    output_path.display()
                );
            }
        }
        let json = serde_json::to_string_pretty(&serde_json::Value::Object(gozen))?;
        std::fs::write(&output_path, json)?;
        println!("Wrote: {}", output_path.display());
    }

    // Summary
    let unmapped: Vec<&str> = config
        .keys()
        .filter(|k| {
            !matches!(
                k.as_str(),
                "max-line-length" | "tab-characters" | "disabled"
            )
        })
        .map(|s| s.as_str())
        .collect();

    println!("\nMigration complete.");
    if !unmapped.is_empty() {
        println!(
            "Note: {} gdlintrc settings have no gozen equivalent: {}",
            unmapped.len(),
            unmapped.join(", ")
        );
    }

    Ok(())
}

/// Parse a gdtoolkit .gdlintrc file (INI-like format).
fn parse_gdlintrc(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

/// Find .gdlintrc by walking up from start_dir.
fn find_gdlintrc(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(".gdlintrc");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Map a gdlint rule name to a (group, gozen_rule_name) pair.
fn map_gdlint_rule(gdlint_rule: &str) -> Option<(&'static str, &'static str)> {
    match gdlint_rule {
        // Style mappings
        "function-name" | "function_name" => Some(("style", "namingConvention")),
        "class-name" | "class_name" => Some(("style", "namingConvention")),
        "sub-class-name" | "sub_class_name" => Some(("style", "namingConvention")),
        "signal-name" | "signal_name" => Some(("style", "namingConvention")),
        "class-variable-name" | "class_variable_name" => Some(("style", "namingConvention")),
        "function-variable-name" | "function_variable_name" => Some(("style", "namingConvention")),
        "constant-name" | "constant_name" => Some(("style", "namingConvention")),
        "enum-name" | "enum_name" => Some(("style", "namingConvention")),
        "enum-element-name" | "enum_element_name" => Some(("style", "namingConvention")),
        "max-line-length" => Some(("style", "lineLength")),
        // Correctness mappings
        "unused-variable" | "unused_variable" => Some(("correctness", "noUnusedVariables")),
        "unused-argument" | "unused_argument" => Some(("correctness", "noUnusedParameter")),
        // Suspicious mappings
        "duplicated-branches" | "duplicated_branches" => Some(("suspicious", "noDuplicateBranch")),
        _ => None,
    }
}
