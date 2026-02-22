//! Format CLI integration test. Run with: cargo test -p gozen --test format_test

use std::fs;
use std::process::Command;

fn gozen() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gozen"))
}

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
    let dir = std::env::temp_dir().join("gozen_format_check_test");
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
    let dir = std::env::temp_dir().join("gozen_format_write_test");
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

#[cfg(unix)]
#[test]
fn test_format_refuses_symlink_target_writes() {
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after UNIX_EPOCH")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("gozen_format_symlink_test_{ts}"));
    fs::create_dir_all(&dir).unwrap();

    let real_file = dir.join("real.gd");
    fs::write(&real_file, "extends Node\nvar x=1\n").unwrap();
    let symlink_file = dir.join("link.gd");
    symlink(&real_file, &symlink_file).unwrap();

    let out = gozen().arg("format").arg(&symlink_file).output().unwrap();
    assert!(
        !out.status.success(),
        "expected failure when writing through symlink.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let content = fs::read_to_string(&real_file).unwrap();
    assert_eq!(content, "extends Node\nvar x=1\n");
}
