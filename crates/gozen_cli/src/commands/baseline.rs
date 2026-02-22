// baseline --create writes gozen-baseline.json

use std::path::PathBuf;

use gozen_config::GozenConfig;
use serde::Serialize;

use super::utils::content_hash_for_diagnostic;

#[derive(Serialize)]
struct BaselineDiagnostic {
    file: String,
    rule: String,
    line: u32,
    column: u32,
    #[serde(rename = "contentHash")]
    content_hash: String,
}

#[derive(Serialize)]
struct BaselineSummary {
    total: usize,
    errors: usize,
    warnings: usize,
}

#[derive(Serialize)]
struct BaselineFile {
    version: u32,
    created: String,
    #[serde(rename = "gozenVersion")]
    gozen_version: String,
    diagnostics: Vec<BaselineDiagnostic>,
    summary: BaselineSummary,
}

/// Run baseline command. --create writes current diagnostics to output path.
pub fn run(
    paths: &[PathBuf],
    create: bool,
    output: &std::path::Path,
    config: &GozenConfig,
    start_dir: &std::path::Path,
) -> anyhow::Result<bool> {
    if !create {
        eprintln!("Use gozen baseline --create . to generate a baseline file.");
        return Ok(true);
    }
    let diags = super::lint::run_collect_diagnostics(paths, config, start_dir)?;
    let (_, format_failures) =
        super::format::run(paths, true, config, start_dir, None, false, false, false)?;
    let total = diags.len() + format_failures;
    let errors = diags
        .iter()
        .filter(|d| matches!(d.severity, gozen_diagnostics::Severity::Error))
        .count();
    let warnings = diags
        .iter()
        .filter(|d| matches!(d.severity, gozen_diagnostics::Severity::Warning))
        .count();

    let diagnostics: Vec<BaselineDiagnostic> = diags
        .iter()
        .map(|d| BaselineDiagnostic {
            file: d.file_path.clone().unwrap_or_default(),
            rule: d.rule_id.clone().unwrap_or_default(),
            line: (d.span.start_row + 1) as u32,
            column: (d.span.start_col + 1) as u32,
            content_hash: content_hash_for_diagnostic(d),
        })
        .collect();

    let created = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );
    let baseline = BaselineFile {
        version: 1,
        created,
        gozen_version: env!("CARGO_PKG_VERSION").to_string(),
        diagnostics,
        summary: BaselineSummary {
            total,
            errors,
            warnings,
        },
    };
    let json = serde_json::to_string_pretty(&baseline)?;
    if let Ok(meta) = std::fs::symlink_metadata(output) {
        if meta.file_type().is_symlink() {
            anyhow::bail!("Refusing to overwrite symlinked path: {}", output.display());
        }
    }
    std::fs::write(output, json)?;
    println!("Created {} with {} diagnostic(s).", output.display(), total);
    Ok(true)
}
