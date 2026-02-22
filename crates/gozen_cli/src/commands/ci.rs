// CI mode — strict exit, --changed, --baseline

use std::path::PathBuf;
use std::process::Command;

use gozen_config::GozenConfig;

use super::utils::content_hash_for_diagnostic;
use crate::discovery::discover_files;
use crate::reporters;

const MAX_BASELINE_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

/// Base branch for git diff: config vcs.defaultBranch or "main"
fn default_base_branch(config: &GozenConfig) -> String {
    config
        .vcs
        .default_branch
        .clone()
        .unwrap_or_else(|| "main".to_string())
}

fn is_ci_target_script(path: &str) -> bool {
    path.ends_with(".gd") || path.ends_with(".gdshader")
}

fn resolve_git_binary() -> anyhow::Result<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    {
        for candidate in ["/usr/bin/git", "/bin/git", "/usr/local/bin/git"] {
            let candidate = PathBuf::from(candidate);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        anyhow::bail!(
            "Could not locate git in trusted locations (/usr/bin/git, /bin/git, /usr/local/bin/git)"
        );
    }

    #[cfg(target_os = "windows")]
    {
        Ok(PathBuf::from("git.exe"))
    }
}

fn is_valid_git_ref(ref_name: &str) -> bool {
    if ref_name.is_empty() || ref_name.len() > 200 {
        return false;
    }
    if ref_name.starts_with('-')
        || ref_name.ends_with('/')
        || ref_name.contains("..")
        || ref_name.contains("@{")
        || ref_name.contains("//")
    {
        return false;
    }
    !ref_name
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '\0'..='\x1f' | ':' | '?' | '*' | '[' | '\\'))
}

/// Get changed script files (.gd, .gdshader) vs base branch and staged.
fn get_changed_files(
    config: &GozenConfig,
    start_dir: &std::path::Path,
) -> anyhow::Result<Vec<PathBuf>> {
    let base = default_base_branch(config);
    if !is_valid_git_ref(&base) {
        anyhow::bail!("Invalid vcs.defaultBranch value: {}", base);
    }
    let git = resolve_git_binary()?;
    let verify = Command::new(&git)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ])
        .current_dir(start_dir)
        .output()?;
    if !verify.status.success() {
        anyhow::bail!(
            "Base branch '{}' was not found or is not a commit-ish",
            base
        );
    }

    let mut files = Vec::new();
    let out = Command::new(&git)
        .args(["diff", "--name-only", "--diff-filter=ACMR", &base])
        .current_dir(start_dir)
        .output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git diff against base branch failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let line = line.trim();
        if is_ci_target_script(line) {
            files.push(PathBuf::from(line));
        }
    }
    let out_staged = Command::new(&git)
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .current_dir(start_dir)
        .output()?;
    if !out_staged.status.success() {
        anyhow::bail!(
            "git diff --cached failed: {}",
            String::from_utf8_lossy(&out_staged.stderr).trim()
        );
    }
    for line in String::from_utf8_lossy(&out_staged.stdout).lines() {
        let line = line.trim();
        if is_ci_target_script(line) && !files.iter().any(|p| p == &PathBuf::from(line)) {
            files.push(PathBuf::from(line));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Run CI: no --write, exit 1 on any diagnostic (or new if baseline), support --changed and --baseline
pub fn run(
    paths: &[PathBuf],
    changed: bool,
    baseline: Option<PathBuf>,
    config: &GozenConfig,
    start_dir: &std::path::Path,
    max_diagnostics: usize,
    reporter: reporters::Reporter,
) -> anyhow::Result<bool> {
    let paths_to_check: Vec<PathBuf> = if changed {
        let changed_files = get_changed_files(config, start_dir)?;
        if changed_files.is_empty() {
            return Ok(true);
        }
        changed_files
    } else {
        paths.to_vec()
    };

    let (format_ok, format_failures) = super::format::run(
        &paths_to_check,
        true,
        config,
        start_dir,
        None,
        false,
        false,
        false,
    )?;
    let all_diags = super::lint::run_collect_diagnostics(&paths_to_check, config, start_dir)?;

    let (diags_to_report, new_count, baselined_count) = if let Some(baseline_path) = &baseline {
        if let Ok(metadata) = std::fs::metadata(baseline_path) {
            if metadata.len() > MAX_BASELINE_FILE_SIZE_BYTES {
                anyhow::bail!(
                    "Baseline file {} is too large ({} bytes, max {} bytes)",
                    baseline_path.display(),
                    metadata.len(),
                    MAX_BASELINE_FILE_SIZE_BYTES
                );
            }
        }
        let baseline_content = match std::fs::read_to_string(baseline_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "Warning: could not read baseline {}: {}",
                    baseline_path.display(),
                    e
                );
                String::new()
            }
        };
        let baseline_entries: Vec<BaselineEntry> = match serde_json::from_str(&baseline_content) {
            Ok(entries) => entries,
            Err(e) => {
                if !baseline_content.is_empty() {
                    eprintln!("Warning: could not parse baseline JSON: {}", e);
                }
                Vec::new()
            }
        };
        let mut new = Vec::new();
        let mut baselined = 0;
        for d in &all_diags {
            let file = d.file_path.as_deref().unwrap_or("");
            let rule = d.rule_id.as_deref().unwrap_or("");
            let content_hash = content_hash_for_diagnostic(d);
            let matched = baseline_entries.iter().any(|e| {
                e.file == file && e.rule == rule && e.content_hash.as_deref() == Some(&content_hash)
            });
            if matched {
                baselined += 1;
            } else {
                new.push(d.clone());
            }
        }
        let n = new.len();
        (new, n, baselined)
    } else {
        let len = all_diags.len();
        (all_diags, len, 0)
    };

    let errors = diags_to_report
        .iter()
        .filter(|d| matches!(d.severity, gozen_diagnostics::Severity::Error))
        .count();
    let warnings = diags_to_report
        .iter()
        .filter(|d| matches!(d.severity, gozen_diagnostics::Severity::Warning))
        .count();
    let summary = reporters::ReportSummary {
        errors,
        warnings,
        files_checked: discover_files(&paths_to_check, config).len(),
        duration_ms: 0,
        format_failures,
    };
    let shown: Vec<_> = diags_to_report.into_iter().take(max_diagnostics).collect();
    reporters::report_diagnostics(&shown, summary, reporter);

    if baseline.is_some() {
        if new_count > 0 || format_failures > 0 {
            eprintln!(
                "\n{} new issue(s) found. {} pre-existing suppressed by baseline.",
                new_count + format_failures,
                baselined_count
            );
            return Ok(false);
        }
        Ok(true)
    } else {
        Ok(format_ok && errors == 0 && warnings == 0 && format_failures == 0)
    }
}

#[derive(serde::Deserialize)]
struct BaselineEntry {
    file: String,
    rule: String,
    #[serde(rename = "contentHash")]
    content_hash: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::is_ci_target_script;

    #[test]
    fn ci_changed_targets_expected_extensions() {
        let cases = [
            ("res://scripts/player.gd", true),
            ("res://shaders/water.gdshader", true),
            ("res://scripts/player.gd.txt", false),
            ("res://scenes/main.tscn", false),
            ("res://shaders/water.GDSHADER", false),
        ];
        for (path, expected) in cases {
            assert_eq!(
                is_ci_target_script(path),
                expected,
                "unexpected match result for {path}"
            );
        }
    }
}
