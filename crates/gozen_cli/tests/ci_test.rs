//! CLI integration tests focused on `gozen ci` behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn gozen() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gozen"))
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after UNIX_EPOCH")
        .as_nanos();
    for attempt in 0..1000_u32 {
        let dir =
            std::env::temp_dir().join(format!("{prefix}_{ts}_{}_{}", std::process::id(), attempt));
        match fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("should create temp dir {dir:?}: {e}"),
        }
    }
    panic!("failed to allocate unique temp dir for {prefix}");
}

fn git(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git command should run")
}

fn assert_git_success(repo: &Path, args: &[&str]) {
    let out = git(repo, args);
    assert!(
        out.status.success(),
        "git {:?} failed.\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_git_repo() -> PathBuf {
    let repo = unique_temp_dir("gozen_ci_test_repo");
    assert_git_success(&repo, &["init", "-b", "main"]);
    assert_git_success(&repo, &["config", "user.email", "qa@example.com"]);
    assert_git_success(&repo, &["config", "user.name", "qa"]);
    fs::write(repo.join("README.md"), "base\n").expect("should write README");
    assert_git_success(&repo, &["add", "README.md"]);
    assert_git_success(&repo, &["commit", "-m", "base"]);
    assert_git_success(&repo, &["checkout", "-b", "feat"]);
    repo
}

fn run_ci_changed(repo: &Path) -> Output {
    gozen()
        .args(["ci", "--changed", "."])
        .current_dir(repo)
        .output()
        .expect("gozen ci should run")
}

#[test]
fn ci_changed_fails_on_staged_gdshader_format_issue() {
    let repo = init_git_repo();
    fs::write(
        repo.join("bad.gdshader"),
        "shader_type spatial;\nvoid fragment(){ALBEDO=vec3(1.0);}\n",
    )
    .expect("should write shader file");
    assert_git_success(&repo, &["add", "bad.gdshader"]);

    let out = run_ci_changed(&repo);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "expected failure; stdout:\n{stdout}");
    assert!(
        stdout.contains("1 file(s) need formatting."),
        "expected format failure summary in stdout:\n{stdout}"
    );
}

#[test]
fn ci_changed_fails_on_staged_gd_format_issue() {
    let repo = init_git_repo();
    fs::write(
        repo.join("bad.gd"),
        "extends   Node\nvar x=1\nfunc _ready(): pass\n",
    )
    .expect("should write gd file");
    assert_git_success(&repo, &["add", "bad.gd"]);

    let out = run_ci_changed(&repo);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "expected failure; stdout:\n{stdout}");
    assert!(
        stdout.contains("1 file(s) need formatting."),
        "expected format failure summary in stdout:\n{stdout}"
    );
}

#[test]
fn ci_changed_ignores_untracked_files() {
    let repo = init_git_repo();
    fs::write(
        repo.join("bad.gd"),
        "extends   Node\nvar x=1\nfunc _ready(): pass\n",
    )
    .expect("should write untracked gd file");

    let out = run_ci_changed(&repo);
    assert!(
        out.status.success(),
        "expected success when only untracked files exist.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ci_changed_returns_success_when_no_changed_tracked_scripts() {
    let repo = init_git_repo();
    fs::write(repo.join("notes.txt"), "not a script change\n").expect("should write notes");
    assert_git_success(&repo, &["add", "notes.txt"]);

    let out = run_ci_changed(&repo);
    assert!(
        out.status.success(),
        "expected success when changed tracked files are non-script.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ci_changed_dedupes_file_present_in_both_diff_and_cached() {
    let repo = init_git_repo();
    fs::write(repo.join("dupe.gd"), "extends Node\n").expect("should write initial file");
    assert_git_success(&repo, &["add", "dupe.gd"]);
    assert_git_success(&repo, &["commit", "-m", "add dupe"]);

    // Make staged change.
    fs::write(
        repo.join("dupe.gd"),
        "extends   Node\nvar x=1\nfunc _ready(): pass\n",
    )
    .expect("should write staged change");
    assert_git_success(&repo, &["add", "dupe.gd"]);

    // Make additional unstaged change so path appears in both diff modes.
    fs::write(
        repo.join("dupe.gd"),
        "extends   Node\nvar x=2\nfunc _ready(): pass\n",
    )
    .expect("should write unstaged change");

    let out = run_ci_changed(&repo);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "expected failure; stdout:\n{stdout}");
    assert!(
        stdout.contains("1 file(s) need formatting."),
        "expected deduped single-file format summary in stdout:\n{stdout}"
    );
}

#[test]
fn ci_changed_with_baseline_suppresses_existing_lint_issue() {
    let repo = init_git_repo();
    let source = "extends Node\n\nfunc _ready() -> void:\n\tvar unused := 1\n";
    fs::write(repo.join("baseline_case.gd"), source).expect("should write gd file");
    assert_git_success(&repo, &["add", "baseline_case.gd"]);

    let baseline_path = repo.join("gozen-baseline.json");
    let baseline_out = gozen()
        .args(["baseline", "--create", "--output"])
        .arg(&baseline_path)
        .arg(".")
        .current_dir(&repo)
        .output()
        .expect("gozen baseline should run");
    assert!(
        baseline_out.status.success(),
        "baseline creation should succeed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&baseline_out.stdout),
        String::from_utf8_lossy(&baseline_out.stderr)
    );

    let out = gozen()
        .args(["ci", "--changed", "--baseline"])
        .arg(&baseline_path)
        .arg(".")
        .current_dir(&repo)
        .output()
        .expect("gozen ci should run");
    assert!(
        out.status.success(),
        "expected success when issue is baselined.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ci_changed_fails_closed_with_invalid_base_branch_config() {
    let repo = init_git_repo();
    fs::write(repo.join("gozen.json"), r#"{"vcs":{"defaultBranch":"bad branch"}}"#)
        .expect("should write config");

    let out = run_ci_changed(&repo);
    assert!(
        !out.status.success(),
        "expected failure for invalid base branch.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Invalid vcs.defaultBranch value"),
        "expected invalid branch error.\nstderr:\n{stderr}"
    );
}
