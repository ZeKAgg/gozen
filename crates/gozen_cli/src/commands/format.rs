use std::path::PathBuf;
use std::time::Instant;

use rayon::prelude::*;
use gozen_config::GozenConfig;
use gozen_formatter::{format as fmt, format_shader};
use gozen_parser::{GDScriptParser, GDShaderParser};

use crate::cache::{self, GozenCache};
use crate::discovery::discover_files;

const MAX_SOURCE_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

/// Result of formatting a single file.
struct FormatResult {
    path: PathBuf,
    source: String,
    formatted: String,
    changed: bool,
}

/// Returns (success, number of files that need formatting when in check mode).
#[allow(clippy::too_many_arguments)]
pub fn run(
    paths: &[PathBuf],
    check: bool,
    config: &GozenConfig,
    _start_dir: &std::path::Path,
    stdin_filepath: Option<PathBuf>,
    verbose: bool,
    diff: bool,
    quiet: bool,
) -> anyhow::Result<(bool, usize)> {
    // Skip formatting if disabled in config
    if !config.formatter.enabled {
        if !quiet {
            println!("Formatter is disabled in config.");
        }
        return Ok((true, 0));
    }

    // Stdin piping mode: read from stdin, write formatted output to stdout
    if let Some(_path) = stdin_filepath {
        let source = std::io::read_to_string(std::io::stdin())?;
        let mut parser = GDScriptParser::new();
        let tree = match parser.parse(&source) {
            Some(t) => t,
            None => {
                // Can't parse — echo source back unchanged
                print!("{}", source);
                return Ok((true, 0));
            }
        };
        let formatted = fmt(&source, &tree, &config.formatter);
        print!("{}", formatted);
        return Ok((true, 0));
    }

    if !check {
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

    let start = Instant::now();
    let files = discover_files(paths, config);
    let formatter_config = &config.formatter;
    let config_hash = cache::hash_config(formatter_config);

    // Load cache if a project root exists
    let project_root = cache::find_project_root(_start_dir);
    let mut file_cache = project_root
        .as_ref()
        .map(|root| GozenCache::load(root))
        .unwrap_or_default();

    // Process files in parallel
    let results: Vec<FormatResult> = files
        .par_iter()
        .filter_map(|path| {
            if let Ok(metadata) = std::fs::metadata(path) {
                if metadata.len() > MAX_SOURCE_FILE_SIZE_BYTES {
                    eprintln!(
                        "Warning: skipped {} ({} bytes exceeds {} byte limit)",
                        path.display(),
                        metadata.len(),
                        MAX_SOURCE_FILE_SIZE_BYTES
                    );
                    return None;
                }
            }
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Warning: could not read {}: {}", path.display(), e);
                    return None;
                }
            };
            let content_hash = cache::hash_content(&source);
            let cache_key = path.to_string_lossy().to_string();

            // Skip if cached (file content and config unchanged)
            if file_cache.is_cached(&cache_key, content_hash, config_hash) {
                return None;
            }

            let is_shader = path.extension().is_some_and(|e| e == "gdshader");
            let formatted = if is_shader {
                let mut parser = GDShaderParser::new();
                let tree = parser.parse(&source)?;
                format_shader(&source, &tree, formatter_config)
            } else {
                let mut parser = GDScriptParser::new();
                let tree = parser.parse(&source)?;
                fmt(&source, &tree, formatter_config)
            };
            let changed = formatted != source;
            Some(FormatResult {
                path: path.clone(),
                source,
                formatted,
                changed,
            })
        })
        .collect();

    let _total_processed = results.len();
    // Cache files that are already formatted (unchanged)
    for result in &results {
        if !result.changed {
            let content_hash = cache::hash_content(&result.source);
            let cache_key = result.path.to_string_lossy().to_string();
            file_cache.update(cache_key, content_hash, config_hash);
        }
    }
    let total = files.len(); // Total files discovered (includes cached + processed)
    let need_format: Vec<_> = results.into_iter().filter(|r| r.changed).collect();
    let format_failures = need_format.len();

    // --diff mode: show unified diff and exit (implies check)
    if diff {
        for result in &need_format {
            print_diff(&result.path, &result.source, &result.formatted);
        }
        if !need_format.is_empty() && !quiet {
            eprintln!(
                "\n{} files need formatting. Run `gozen format .` to fix.",
                format_failures
            );
        }
        if !quiet {
            eprintln!("Done in {:.2}s.", start.elapsed().as_secs_f64());
        }
        // Save cache (for files that were already formatted)
        if let Some(root) = &project_root {
            let _ = file_cache.save(root);
        }
        return Ok((need_format.is_empty(), format_failures));
    }

    if check {
        for result in &need_format {
            if !quiet {
                eprintln!("{} — not formatted", result.path.display());
            }
        }
        // Save cache (for files that were already formatted)
        if let Some(root) = &project_root {
            let _ = file_cache.save(root);
        }
        if !need_format.is_empty() && !quiet {
            eprintln!(
                "\n{} file(s) need formatting. Run `gozen format .` to fix.",
                format_failures
            );
            return Ok((false, format_failures));
        }
        return Ok((true, 0));
    }

    // Write mode
    let changed = format_failures;
    for result in need_format {
        if verbose && !quiet {
            println!("  {} (changed)", result.path.display());
        }
        let metadata = std::fs::symlink_metadata(&result.path)?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!(
                "Refusing to write through symlinked path: {}",
                result.path.display()
            );
        }
        // Update cache with the formatted content hash
        let formatted_hash = cache::hash_content(&result.formatted);
        let cache_key = result.path.to_string_lossy().to_string();
        file_cache.update(cache_key, formatted_hash, config_hash);
        std::fs::write(&result.path, result.formatted)?;
    }
    if !quiet && !files.is_empty() {
        let elapsed = start.elapsed().as_secs_f64();
        if changed > 0 {
            println!(
                "Formatted {} files ({} changed) in {:.2}s.",
                total, changed, elapsed
            );
        } else {
            println!("All {} files already formatted in {:.2}s.", total, elapsed);
        }
    }

    // Save cache
    if let Some(root) = &project_root {
        let _ = file_cache.save(root);
    }

    Ok((true, 0))
}

/// Print a unified diff between original and formatted content.
fn print_diff(path: &std::path::Path, original: &str, formatted: &str) {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(original, formatted);
    let path_str = path.display().to_string();

    println!("--- {}", path_str);
    println!("+++ {}", path_str);

    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        println!("{}", hunk.header());
        for change in hunk.iter_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            print!("{}{}", sign, change);
            if change.missing_newline() {
                println!();
            }
        }
    }
}
