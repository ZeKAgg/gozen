//! Shared test utilities for workspace-level integration tests.
//!
//! Usage: add `mod common;` at the top of your test file.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

/// Locate the gozen binary built by cargo.
///
/// Checks `CARGO_BIN_EXE_gozen` first (set by cargo test for binary crates),
/// then falls back to `target/debug/gozen` (with `.exe` on Windows).
pub fn gozen_bin() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_gozen") {
        return PathBuf::from(path);
    }
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| format!("{}/target", env!("CARGO_MANIFEST_DIR")));
    let stem = PathBuf::from(&target_dir).join("debug/gozen");
    if cfg!(windows) {
        stem.with_extension("exe")
    } else {
        stem
    }
}

/// Create a `Command` for the gozen binary.
pub fn gozen() -> Command {
    Command::new(gozen_bin())
}

/// Path to the workspace-level test fixtures directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
