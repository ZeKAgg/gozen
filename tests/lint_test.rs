//! Lint CLI integration tests.
//! Run with: cargo test -p gozen_workspace lint_test

mod common;

use common::{fixtures_dir, gozen};

#[test]
fn test_lint_help() {
    let out = gozen().args(["lint", "--help"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("lint"));
    assert!(stdout.contains("--write"));
}

#[test]
fn test_lint_reports_diagnostics() {
    let file = fixtures_dir().join("lint/no_unused_variables/fail/simple_unused.gd");
    let out = gozen()
        .args(["lint", "--reporter=json"])
        .arg(&file)
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("lint --reporter=json should output valid JSON");
    assert!(
        parsed.get("diagnostics").is_some(),
        "output should have diagnostics key; got: {}",
        stdout
    );
    assert!(
        parsed.get("summary").is_some(),
        "output should have summary key; got: {}",
        stdout
    );
}

#[test]
fn test_lint_json_reporter() {
    let file = fixtures_dir().join("lint/no_unused_variables/fail/simple_unused.gd");
    let out = gozen()
        .args(["lint", "--reporter=json"])
        .arg(&file)
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .is_some());
}

#[test]
fn test_lint_json_reports_complexity_rules_when_enabled() {
    let fixture_dir = fixtures_dir().join("lint/complexity");
    let gd_file = fixture_dir.join("complexity.gd");
    let shader_file = fixture_dir.join("complexity.gdshader");
    let config = fixture_dir.join("config.json");

    let out = gozen()
        .args(["lint", "--reporter=json", "--config"])
        .arg(&config)
        .arg(&gd_file)
        .arg(&shader_file)
        .output()
        .unwrap();

    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let rules: Vec<String> = parsed
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .unwrap()
        .iter()
        .filter_map(|d| d.get("rule").and_then(|r| r.as_str()).map(str::to_owned))
        .collect();

    assert!(
        rules.iter().any(|r| r == "style/cognitiveComplexity"),
        "expected style/cognitiveComplexity in diagnostics, got: {:?}",
        rules
    );
    assert!(
        rules.iter().any(|r| r == "style/cyclomaticComplexity"),
        "expected style/cyclomaticComplexity in diagnostics, got: {:?}",
        rules
    );
    assert!(
        rules.iter().any(|r| r == "shader/cognitiveComplexity"),
        "expected shader/cognitiveComplexity in diagnostics, got: {:?}",
        rules
    );
    assert!(
        rules.iter().any(|r| r == "shader/cyclomaticComplexity"),
        "expected shader/cyclomaticComplexity in diagnostics, got: {:?}",
        rules
    );
}

#[test]
fn test_explain_complexity_rules() {
    let out = gozen()
        .args(["explain", "cognitiveComplexity"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("cognitiveComplexity"));

    let out = gozen()
        .args(["explain", "cyclomaticComplexity"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("cyclomaticComplexity"));
}
