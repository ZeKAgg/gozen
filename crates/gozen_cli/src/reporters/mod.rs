pub mod github;
pub mod json;
pub mod text;

use clap::ValueEnum;
use gozen_diagnostics::Diagnostic;

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Reporter {
    Text,
    Json,
    Github,
}

pub struct ReportSummary {
    pub errors: usize,
    pub warnings: usize,
    pub files_checked: usize,
    pub duration_ms: u64,
    pub format_failures: usize,
}

pub fn report_diagnostics(diagnostics: &[Diagnostic], summary: ReportSummary, reporter: Reporter) {
    match reporter {
        Reporter::Text => text::report(diagnostics, summary),
        Reporter::Json => json::report(diagnostics, summary),
        Reporter::Github => github::report(diagnostics, summary),
    }
}
