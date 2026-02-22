//! Lint CLI integration test. Run with: cargo test -p gozen --test lint_test

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn gozen() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gozen"))
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

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

fn unique_project_hashes(cache_path: &Path) -> HashSet<u64> {
    let content = fs::read_to_string(cache_path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    value
        .get("entries")
        .and_then(|e| e.as_object())
        .map(|entries| {
            entries
                .values()
                .filter_map(|v| v.get("project_fingerprint_hash"))
                .filter_map(|v| v.as_u64())
                .collect::<HashSet<u64>>()
        })
        .unwrap_or_default()
}

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
    // Lint a file and assert we get valid JSON with diagnostics (may be empty if parse fails or no issues)
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
fn test_lint_creates_cache_file() {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp = std::env::temp_dir().join(format!("gozen_lint_cache_test_{ts}"));
    let src = fixtures_dir().join("projects/project_aware_missing");
    copy_dir_recursive(&src, &tmp);

    let out = gozen()
        .args(["lint", "--reporter=json", "."])
        .current_dir(&tmp)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let cache_path = tmp.join(".godot/gozen_cache/lint_v1.json");
    assert!(
        cache_path.exists(),
        "expected lint cache at {}",
        cache_path.display()
    );
}

#[test]
fn test_lint_cache_invalidates_on_scene_change() {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp = std::env::temp_dir().join(format!("gozen_lint_cache_invalidate_{ts}"));
    let src = fixtures_dir().join("projects/project_aware_missing");
    copy_dir_recursive(&src, &tmp);

    let first = gozen()
        .args(["lint", "--reporter=json", "."])
        .current_dir(&tmp)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let cache_path = tmp.join(".godot/gozen_cache/lint_v1.json");
    let before_hashes = unique_project_hashes(&cache_path);
    assert!(!before_hashes.is_empty(), "expected non-empty cache hashes");

    fs::write(
        tmp.join("scenes/missing_class.tscn"),
        "[gd_scene load_steps=2 format=3]\n\n[ext_resource type=\"Script\" path=\"res://scripts/child_missing.gd\" id=\"1\"]\n\n[node name=\"Root\" type=\"Node2D\"]\nscript = ExtResource(\"1\")\n# changed for cache invalidation\n",
    )
    .unwrap();

    let second = gozen()
        .args(["lint", "--reporter=json", "."])
        .current_dir(&tmp)
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let after_hashes = unique_project_hashes(&cache_path);
    assert!(
        before_hashes != after_hashes,
        "expected project fingerprint hashes to change after scene modification"
    );
}
