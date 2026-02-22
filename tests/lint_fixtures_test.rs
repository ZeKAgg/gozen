//! Lint fixture tests: tests/fixtures/lint/<rule>/{pass,fail}/*.gd
//!
//! For each rule directory under tests/fixtures/lint/:
//!   - `pass/*.gd`: the linter must produce zero diagnostics for the rule.
//!   - `fail/*.gd`: the linter must produce diagnostics matching
//!     the corresponding `*.diagnostics.json` file.
//!

use std::fs;
use std::path::{Path, PathBuf};

use gozen_config::LinterConfig;
use gozen_parser::GDScriptParser;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lint")
}

/// Expected diagnostic from a `.diagnostics.json` file.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ExpectedDiagnostic {
    rule: String,
    severity: String,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
    message_contains: String,
}

#[test]
fn test_all_lint_fixtures() {
    let base = fixtures_dir();
    if !base.exists() {
        return;
    }

    let mut failures: Vec<String> = Vec::new();

    for entry in fs::read_dir(&base).unwrap().flatten() {
        let rule_dir = entry.path();
        if !rule_dir.is_dir()
            || rule_dir
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(true)
        {
            continue;
        }

        // Test pass cases
        let pass_dir = rule_dir.join("pass");
        if pass_dir.exists() {
            for gd_entry in fs::read_dir(&pass_dir).unwrap().flatten() {
                let gd_path = gd_entry.path();
                if gd_path.extension().and_then(|e| e.to_str()) != Some("gd") {
                    continue;
                }
                if let Err(msg) = run_pass_fixture(&gd_path, &rule_dir) {
                    failures.push(msg);
                }
            }
        }

        // Test fail cases
        let fail_dir = rule_dir.join("fail");
        if fail_dir.exists() {
            for gd_entry in fs::read_dir(&fail_dir).unwrap().flatten() {
                let gd_path = gd_entry.path();
                if gd_path.extension().and_then(|e| e.to_str()) != Some("gd") {
                    continue;
                }
                if let Err(msg) = run_fail_fixture(&gd_path, &rule_dir) {
                    failures.push(msg);
                }
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Lint fixture failures ({}):\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}

/// Pass fixtures: linting should produce zero diagnostics for the rule under test.
fn run_pass_fixture(gd_path: &Path, rule_dir: &Path) -> Result<(), String> {
    let rule_name = rule_dir.file_name().unwrap().to_string_lossy().to_string();

    let source = fs::read_to_string(gd_path)
        .map_err(|e| format!("[{}] Failed to read {:?}: {}", rule_name, gd_path, e))?;

    let config = LinterConfig::default();
    let engine = gozen_linter::LintEngine::new(&config);
    let mut parser = GDScriptParser::new();

    let tree = parser
        .parse(&source)
        .ok_or_else(|| format!("[{}] Failed to parse {:?}", rule_name, gd_path))?;

    let file_str = gd_path.to_string_lossy();
    let diags = engine.lint(&tree, &source, &file_str, None, None, None);

    // Filter to only diagnostics matching this rule (by converting rule dir name
    // to the rule ID pattern, e.g. "no_unused_variables" -> "noUnusedVariables")
    let rule_id_suffix = dir_name_to_rule_name(&rule_name);
    let relevant: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.rule_id
                .as_ref()
                .map(|id| id.ends_with(&rule_id_suffix))
                .unwrap_or(false)
        })
        .collect();

    if !relevant.is_empty() {
        return Err(format!(
            "[{}] PASS fixture {:?} should have zero diagnostics for rule, but got {}:\n{}",
            rule_name,
            gd_path.file_name().unwrap(),
            relevant.len(),
            relevant
                .iter()
                .map(|d| format!(
                    "  - {} ({}:{})",
                    d.message, d.span.start_row, d.span.start_col
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Ok(())
}

/// Fail fixtures: linting should produce diagnostics matching the `.diagnostics.json` file.
fn run_fail_fixture(gd_path: &Path, rule_dir: &Path) -> Result<(), String> {
    let rule_name = rule_dir.file_name().unwrap().to_string_lossy().to_string();

    let diag_json_path = gd_path.with_extension("diagnostics.json");
    if !diag_json_path.exists() {
        return Err(format!(
            "[{}] Missing diagnostics.json for {:?}",
            rule_name,
            gd_path.file_name().unwrap()
        ));
    }

    let source = fs::read_to_string(gd_path)
        .map_err(|e| format!("[{}] Failed to read {:?}: {}", rule_name, gd_path, e))?;

    let expected_json = fs::read_to_string(&diag_json_path)
        .map_err(|e| format!("[{}] Failed to read {:?}: {}", rule_name, diag_json_path, e))?;

    let expected: Vec<ExpectedDiagnostic> = serde_json::from_str(&expected_json).map_err(|e| {
        format!(
            "[{}] Failed to parse {:?}: {}",
            rule_name, diag_json_path, e
        )
    })?;

    let config = LinterConfig::default();
    let engine = gozen_linter::LintEngine::new(&config);
    let mut parser = GDScriptParser::new();

    let tree = parser
        .parse(&source)
        .ok_or_else(|| format!("[{}] Failed to parse {:?}", rule_name, gd_path))?;

    let file_str = gd_path.to_string_lossy();
    let diags = engine.lint(&tree, &source, &file_str, None, None, None);

    // For each expected diagnostic, find a matching actual diagnostic
    let mut errors: Vec<String> = Vec::new();

    for (i, exp) in expected.iter().enumerate() {
        let matching = diags.iter().find(|d| {
            let rule_matches = d
                .rule_id
                .as_ref()
                .map(|id| id == &exp.rule)
                .unwrap_or(false);

            let severity_matches = match exp.severity.as_str() {
                "error" => d.severity == gozen_diagnostics::Severity::Error,
                "warning" => d.severity == gozen_diagnostics::Severity::Warning,
                "info" => d.severity == gozen_diagnostics::Severity::Info,
                _ => false,
            };

            // Both tree-sitter and fixture JSON use 0-indexed lines.
            let line_matches = d.span.start_row == exp.start_line;
            let message_matches = d.message.contains(&exp.message_contains);

            rule_matches && severity_matches && line_matches && message_matches
        });

        if matching.is_none() {
            let actual_for_rule: Vec<_> = diags
                .iter()
                .filter(|d| {
                    d.rule_id
                        .as_ref()
                        .map(|id| id == &exp.rule)
                        .unwrap_or(false)
                })
                .collect();

            errors.push(format!(
                "  expected[{}]: rule={}, severity={}, line={}, messageContains=\"{}\"\n  \
                 actual diagnostics for rule ({}):\n{}",
                i,
                exp.rule,
                exp.severity,
                exp.start_line,
                exp.message_contains,
                actual_for_rule.len(),
                actual_for_rule
                    .iter()
                    .map(|d| format!(
                        "    - [{}:{}] {:?}: {}",
                        d.span.start_row + 1,
                        d.span.start_col,
                        d.severity,
                        d.message
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }

    if !errors.is_empty() {
        return Err(format!(
            "[{}] FAIL fixture {:?} diagnostic mismatch:\n{}",
            rule_name,
            gd_path.file_name().unwrap(),
            errors.join("\n")
        ));
    }

    Ok(())
}

/// Convert a snake_case directory name (e.g., "no_unused_variables") to a
/// camelCase rule name suffix (e.g., "noUnusedVariables").
fn dir_name_to_rule_name(dir_name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in dir_name.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod dir_name_tests {
    use super::*;

    #[test]
    fn test_dir_name_to_rule_name() {
        assert_eq!(
            dir_name_to_rule_name("no_unused_variables"),
            "noUnusedVariables"
        );
        assert_eq!(
            dir_name_to_rule_name("naming_convention"),
            "namingConvention"
        );
        assert_eq!(
            dir_name_to_rule_name("no_unreachable_code"),
            "noUnreachableCode"
        );
    }
}
