use gozen_diagnostics::render_diagnostic;

use super::ReportSummary;

pub fn report(diagnostics: &[gozen_diagnostics::Diagnostic], summary: ReportSummary) {
    for d in diagnostics {
        println!("{}", render_diagnostic(d, None));
    }
    if !diagnostics.is_empty() {
        let elapsed_s = summary.duration_ms as f64 / 1000.0;
        println!(
            "\nFound {} warnings and {} errors in {} files in {:.2}s.",
            summary.warnings, summary.errors, summary.files_checked, elapsed_s
        );
    }
    if summary.format_failures > 0 {
        println!("{} file(s) need formatting.", summary.format_failures);
    }
}
