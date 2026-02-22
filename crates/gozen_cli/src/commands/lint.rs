use std::path::PathBuf;
use std::time::Instant;

use gozen_config::GozenConfig;
use gozen_diagnostics::{Diagnostic, TextEdit};
use gozen_linter::{LintContext, LintEngine};
use gozen_parser::{GDScriptParser, GDShaderParser};
use gozen_project::ProjectGraph;

use crate::discovery::discover_files;
use crate::reporters;
use crate::{cache, cache::LintCache};

const MAX_SOURCE_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

fn read_source_file(path: &std::path::Path) -> std::io::Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_SOURCE_FILE_SIZE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "file is too large ({} bytes, max {})",
                metadata.len(),
                MAX_SOURCE_FILE_SIZE_BYTES
            ),
        ));
    }
    std::fs::read_to_string(path)
}

fn to_script_res_path(path: &std::path::Path, root: &std::path::Path) -> Option<String> {
    if let Ok(rel) = path.strip_prefix(root) {
        return Some(format!(
            "res://{}",
            rel.to_string_lossy().replace('\\', "/")
        ));
    }
    let joined = root.join(path);
    let rel = joined.strip_prefix(root).ok()?;
    Some(format!(
        "res://{}",
        rel.to_string_lossy().replace('\\', "/")
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    paths: &[PathBuf],
    write: bool,
    config: &GozenConfig,
    max_diagnostics: usize,
    reporter: reporters::Reporter,
    format_failures: usize,
    start_dir: &std::path::Path,
    quiet: bool,
) -> anyhow::Result<bool> {
    if write {
        for path in paths {
            if !path.is_file() {
                continue;
            }
            let metadata = std::fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink()
                && path
                    .extension()
                    .is_some_and(|e| e == "gd" || e == "gdshader")
            {
                anyhow::bail!("Refusing to write through symlinked path: {}", path.display());
            }
        }
    }

    let files = discover_files(paths, config);
    let context = LintContext {
        project_root: Some(start_dir.to_path_buf()),
    };
    let start = Instant::now();
    let (all_diags, _) = if write {
        run_lint_and_apply_fixes(&files, config, &context)?
    } else {
        run_lint_once(&files, config, &context)?
    };
    let duration_ms = start.elapsed().as_millis() as u64;
    let errors = all_diags
        .iter()
        .filter(|d| matches!(d.severity, gozen_diagnostics::Severity::Error))
        .count();
    let warnings = all_diags
        .iter()
        .filter(|d| matches!(d.severity, gozen_diagnostics::Severity::Warning))
        .count();
    let summary = reporters::ReportSummary {
        errors,
        warnings,
        files_checked: files.len(),
        duration_ms,
        format_failures,
    };
    if !quiet {
        let shown: Vec<_> = all_diags.into_iter().take(max_diagnostics).collect();
        reporters::report_diagnostics(&shown, summary, reporter);
    }
    Ok(errors == 0)
}

/// Run lint only and return all diagnostics (no reporting, no write). Used by CI.
pub fn run_collect_diagnostics(
    paths: &[PathBuf],
    config: &GozenConfig,
    start_dir: &std::path::Path,
) -> anyhow::Result<Vec<Diagnostic>> {
    let files = discover_files(paths, config);
    let context = LintContext {
        project_root: Some(start_dir.to_path_buf()),
    };
    let (diags, _) = run_lint_once(&files, config, &context)?;
    Ok(diags)
}

fn run_lint_once(
    files: &[PathBuf],
    config: &GozenConfig,
    context: &LintContext,
) -> anyhow::Result<(Vec<Diagnostic>, ())> {
    let project_root = context
        .project_root
        .as_deref()
        .and_then(cache::find_project_root);
    let lint_config_hash = cache::hash_lint_config(config);
    let project_fingerprint_hash = if config.analyzer.project_graph {
        project_root
            .as_ref()
            .map(|root| cache::compute_project_fingerprint(root))
            .unwrap_or(0)
    } else {
        0
    };
    let mut lint_cache = project_root
        .as_ref()
        .map(|root| LintCache::load(root))
        .unwrap_or_default();

    let engine = LintEngine::new_full(
        &config.linter,
        config.analyzer.project_graph,
        &config.shader,
    );
    let mut graph: Option<ProjectGraph> = None;
    let mut parser = GDScriptParser::new();
    let mut shader_parser = GDShaderParser::new();
    let mut all_diags = Vec::new();

    for path in files {
        let source = match read_source_file(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("warning: could not read {}: {}", path.display(), e);
                continue;
            }
        };
        let content_hash = cache::hash_content(&source);
        let cache_key = path.to_string_lossy().to_string();
        if let Some(cached) = lint_cache.lookup(
            &cache_key,
            content_hash,
            lint_config_hash,
            project_fingerprint_hash,
        ) {
            all_diags.extend(cached);
            continue;
        }

        let is_shader = path.extension().is_some_and(|e| e == "gdshader");
        let diags = if is_shader {
            let tree = match shader_parser.parse(&source) {
                Some(t) => t,
                None => continue,
            };
            engine.lint_shader(&tree, &source, &path.to_string_lossy())
        } else {
            let tree = match parser.parse(&source) {
                Some(t) => t,
                None => continue,
            };
            if graph.is_none() && config.analyzer.project_graph {
                graph = project_root
                    .as_ref()
                    .and_then(|root| ProjectGraph::build(root).ok());
            }
            let script_res_path = project_root
                .as_ref()
                .and_then(|root| to_script_res_path(path, root));
            engine.lint(
                &tree,
                &source,
                &path.to_string_lossy(),
                Some(context),
                graph.as_ref(),
                script_res_path.as_deref(),
            )
        };

        lint_cache.update(
            cache_key,
            content_hash,
            lint_config_hash,
            project_fingerprint_hash,
            &diags,
        );
        all_diags.extend(diags);
    }

    if let Some(root) = &project_root {
        let _ = lint_cache.save(root);
    }
    Ok((all_diags, ()))
}

fn run_lint_and_apply_fixes(
    files: &[PathBuf],
    config: &GozenConfig,
    context: &LintContext,
) -> anyhow::Result<(Vec<Diagnostic>, ())> {
    use std::collections::HashMap;

    const MAX_FIX_PASSES: u32 = 5;
    let project_root = context
        .project_root
        .as_deref()
        .and_then(cache::find_project_root);
    let graph: Option<ProjectGraph> = project_root
        .as_ref()
        .filter(|_| config.analyzer.project_graph)
        .and_then(|root| ProjectGraph::build(root).ok());
    let engine = LintEngine::new_full(
        &config.linter,
        config.analyzer.project_graph,
        &config.shader,
    );
    let mut parser = GDScriptParser::new();
    let mut shader_parser = GDShaderParser::new();

    // Read all files into memory once
    let mut sources: HashMap<&PathBuf, String> = HashMap::new();
    let mut original_sources: HashMap<&PathBuf, String> = HashMap::new();
    for path in files {
        match read_source_file(path) {
            Ok(s) => {
                original_sources.insert(path, s.clone());
                sources.insert(path, s);
            }
            Err(e) => {
                eprintln!("warning: could not read {}: {}", path.display(), e);
            }
        }
    }

    // Apply fix passes in memory (no disk I/O between passes)
    for _pass in 0..MAX_FIX_PASSES {
        let mut any_applied = false;
        for path in files {
            let source = match sources.get(path) {
                Some(s) => s.clone(),
                None => continue,
            };
            let is_shader = path.extension().is_some_and(|e| e == "gdshader");
            let diags = if is_shader {
                let tree = match shader_parser.parse(&source) {
                    Some(t) => t,
                    None => continue,
                };
                engine.lint_shader(&tree, &source, &path.to_string_lossy())
            } else {
                let tree = match parser.parse(&source) {
                    Some(t) => t,
                    None => continue,
                };
                let script_res_path = project_root
                    .as_ref()
                    .and_then(|root| to_script_res_path(path, root));
                engine.lint(
                    &tree,
                    &source,
                    &path.to_string_lossy(),
                    Some(context),
                    graph.as_ref(),
                    script_res_path.as_deref(),
                )
            };
            let fixable: Vec<_> = diags
                .iter()
                .filter(|d| {
                    d.fix
                        .as_ref()
                        .is_some_and(|f| f.is_safe && !f.changes.is_empty())
                })
                .cloned()
                .collect();
            if fixable.is_empty() {
                continue;
            }
            let edits: Vec<TextEdit> = fixable
                .iter()
                .filter_map(|d| d.fix.as_ref())
                .flat_map(|f| f.changes.clone())
                .collect();
            if let Some(new_source) = apply_edits(&source, &edits) {
                sources.insert(path, new_source);
                any_applied = true;
            }
        }
        if !any_applied {
            break;
        }
    }

    // Write modified files to disk once
    for path in files {
        if let Some(source) = sources.get(path) {
            let original = original_sources.get(path).map(String::as_str).unwrap_or("");
            if *source != original {
                let metadata = std::fs::symlink_metadata(path)?;
                if metadata.file_type().is_symlink() {
                    anyhow::bail!("Refusing to write through symlinked path: {}", path.display());
                }
                std::fs::write(path, source)?;
            }
        }
    }

    run_lint_once(files, config, context)
}

fn apply_edits(source: &str, edits: &[TextEdit]) -> Option<String> {
    let mut sorted: Vec<_> = edits.iter().collect();
    // Sort by start_byte descending so we apply from end-to-start (preserving earlier offsets)
    sorted.sort_by_key(|e| std::cmp::Reverse(e.span.start_byte));

    let mut out = source.to_string();
    // Track the lowest byte position we've modified so far to detect overlaps
    let mut next_safe_end = usize::MAX;
    for e in sorted {
        let start = e.span.start_byte.min(out.len());
        let end = e.span.end_byte.min(out.len());
        if start > end {
            continue;
        }
        // Skip this edit if it overlaps with a previously applied edit
        if end > next_safe_end {
            continue;
        }
        // Validate that start and end fall on UTF-8 char boundaries
        if !out.is_char_boundary(start) || !out.is_char_boundary(end) {
            eprintln!(
                "Warning: skipping edit at byte range {}..{} — not on char boundary",
                start, end
            );
            continue;
        }
        out = format!("{}{}{}", &out[..start], e.new_text, &out[end..]);
        next_safe_end = start;
    }
    Some(out)
}
