//! CLI check integration tests. See docs/11-TESTING-STRATEGY.md
//! Run with: cargo test -p gozen_workspace check_test

mod common;

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use common::{fixtures_dir, gozen};

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

#[test]
fn test_help() {
    let out = gozen().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("gozen"));
    assert!(stdout.contains("format"));
    assert!(stdout.contains("lint"));
    assert!(stdout.contains("check"));
}

#[test]
fn test_version() {
    let out = gozen().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("0.1.0"));
}

#[test]
fn test_init_creates_config() {
    let dir = std::env::temp_dir().join("gozen_init_test_ws");
    let _ = std::fs::create_dir_all(&dir);
    let out = gozen().arg("init").current_dir(&dir).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("gozen.json").exists());
    let _ = std::fs::remove_file(dir.join("gozen.json"));
}

#[test]
fn test_init_refuses_overwrite_without_force() {
    let dir = std::env::temp_dir().join("gozen_init_no_overwrite_ws");
    let _ = std::fs::create_dir_all(&dir);
    let config_path = dir.join("gozen.json");
    std::fs::write(&config_path, "{}").unwrap();
    let out = gozen().arg("init").current_dir(&dir).output().unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let _ = std::fs::remove_file(config_path);
}

#[test]
fn test_explain_known_rule() {
    let out = gozen()
        .arg("explain")
        .arg("noUnusedVariables")
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("noUnusedVariables"));
    assert!(stdout.contains("correctness"));
}

#[test]
fn test_explain_unknown_rule() {
    let out = gozen()
        .arg("explain")
        .arg("nonexistentRule123")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn test_check_empty_dir_succeeds() {
    let dir = std::env::temp_dir().join("gozen_check_empty_ws");
    let _ = std::fs::create_dir_all(&dir);
    let out = gozen()
        .arg("check")
        .arg(".")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn test_check_simple_project_succeeds() {
    let project = fixtures_dir().join("projects/simple_project");
    let out = gozen().arg("check").arg(&project).output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn test_check_with_lint_issues_reports_diagnostics() {
    let file = fixtures_dir().join("lint/no_unused_variables/fail/simple_unused.gd");
    let out = gozen().arg("check").arg(&file).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let has_diagnostics = stdout.contains("noUnusedVariables")
        || stdout.contains("unused")
        || stdout.contains("need formatting");
    assert!(
        has_diagnostics,
        "check should report format or lint issues; got: {}",
        stdout
    );
}

#[test]
fn test_check_reporter_json() {
    let file = fixtures_dir().join("lint/no_unused_variables/fail/simple_unused.gd");
    let out = gozen()
        .args(["check", "--reporter=json"])
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
fn test_check_reports_cross_scene_parent_contract_rules() {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let project = std::env::temp_dir().join(format!("gozen_cross_scene_ws_{ts}"));
    let src = fixtures_dir().join("projects/cross_scene_contracts");
    copy_dir_recursive(&src, &project);

    let out = gozen()
        .args(["check", "--reporter=json", "."])
        .current_dir(&project)
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let diags = parsed
        .get("diagnostics")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    let mut has_node_contract = false;
    let mut has_signal_contract = false;
    let mut has_method_contract = false;
    let mut has_ambiguous_method = false;
    let mut has_non_root_contract = false;
    let mut has_parent_traversal_contract = false;
    let mut has_unsupported_path_diagnostic = false;

    for d in &diags {
        let rule = d.get("rule").and_then(|r| r.as_str()).unwrap_or_default();
        let message = d
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or_default();
        if rule == "correctness/missingParentNodeContract" {
            has_node_contract = true;
            if message.contains("ExpectedUnderAnchor")
                || message.contains("host_non_root_fail")
                || message.contains("host_non_root_pass")
            {
                has_non_root_contract = true;
            }
            if message.contains("../Sibling")
                || message.contains("host_traversal_fail")
                || message.contains("host_traversal_pass")
            {
                has_parent_traversal_contract = true;
            }
            if message.contains("%UniqueName") || message.contains("/root/World") {
                has_unsupported_path_diagnostic = true;
            }
        }
        if rule == "correctness/missingParentSignalContract" {
            has_signal_contract = true;
            if message.contains("%UniqueName") || message.contains("/root/World") {
                has_unsupported_path_diagnostic = true;
            }
        }
        if rule == "correctness/missingParentMethodContract" {
            has_method_contract = true;
            if message.contains("maybe_method") {
                has_ambiguous_method = true;
            }
            if message.contains("%UniqueName") || message.contains("/root/World") {
                has_unsupported_path_diagnostic = true;
            }
        }
    }

    assert!(
        has_node_contract,
        "missingParentNodeContract was not reported: {stdout}"
    );
    assert!(
        has_signal_contract,
        "missingParentSignalContract was not reported: {stdout}"
    );
    assert!(
        has_method_contract,
        "missingParentMethodContract was not reported: {stdout}"
    );
    assert!(
        !has_non_root_contract,
        "non-root mixed pass/fail host cases should be skipped unless missing in every host: {stdout}"
    );
    assert!(
        !has_parent_traversal_contract,
        "../ traversal mixed pass/fail host cases should be skipped unless missing in every host: {stdout}"
    );
    assert!(
        !has_ambiguous_method,
        "ambiguous maybe_method should not be reported: {stdout}"
    );
    assert!(
        !has_unsupported_path_diagnostic,
        "unsupported path forms should not be reported: {stdout}"
    );
}
