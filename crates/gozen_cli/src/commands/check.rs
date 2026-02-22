use std::path::PathBuf;
use std::time::Instant;

use gozen_config::GozenConfig;

use crate::reporters::Reporter;

pub fn run(
    paths: &[PathBuf],
    write: bool,
    config: &GozenConfig,
    start_dir: &std::path::Path,
    max_diagnostics: usize,
    reporter: Reporter,
    quiet: bool,
) -> anyhow::Result<bool> {
    let start = Instant::now();

    let (format_ok, format_failures) = if write {
        // When --write is set, actually format the files (not just check)
        super::format::run(paths, false, config, start_dir, None, false, false, quiet)?
    } else {
        super::format::run(paths, true, config, start_dir, None, false, false, quiet)?
    };
    let lint_ok = super::lint::run(
        paths,
        write,
        config,
        max_diagnostics,
        reporter,
        format_failures,
        start_dir,
        quiet,
    )?;

    if !quiet {
        let elapsed = start.elapsed();
        eprintln!("Done in {:.2}s.", elapsed.as_secs_f64());
    }

    Ok(lint_ok && format_ok && format_failures == 0)
}
