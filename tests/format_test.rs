//! Format CLI integration tests.
//! Run with: cargo test -p gozen_workspace format_test

mod common;

use std::fs;

use common::gozen;

#[test]
fn test_format_help() {
    let out = gozen().args(["format", "--help"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("format"));
    assert!(stdout.contains("--check"));
}

#[test]
fn test_format_check_mode_reports_unformatted() {
    let dir = std::env::temp_dir().join("gozen_format_check_ws");
    let _ = fs::create_dir_all(&dir);
    let gd = dir.join("foo.gd");
    fs::write(&gd, "extends   Node\nvar x=1\nfunc _ready(): pass\n").unwrap();
    let out = gozen()
        .args(["format", "--check", gd.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("not formatted"));
}

#[test]
fn test_format_writes_formatted_output() {
    let dir = std::env::temp_dir().join("gozen_format_write_ws");
    let _ = fs::create_dir_all(&dir);
    let gd = dir.join("bar.gd");
    let unformatted = "extends   Node\nvar x=1\n";
    fs::write(&gd, unformatted).unwrap();
    let out = gozen().arg("format").arg(&gd).output().unwrap();
    assert!(out.status.success());
    let content = fs::read_to_string(&gd).unwrap();
    assert!(
        content.contains("extends"),
        "formatter should produce extends; got: {:?}",
        content
    );
    assert!(
        content.contains("var"),
        "formatter should produce var; got: {:?}",
        content
    );
    assert_ne!(
        content.trim(),
        unformatted.trim(),
        "formatter should have changed the file"
    );
}
