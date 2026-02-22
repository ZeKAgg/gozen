use super::ReportSummary;

fn severity_cmd(severity: gozen_diagnostics::Severity) -> &'static str {
    match severity {
        gozen_diagnostics::Severity::Error => "error",
        gozen_diagnostics::Severity::Warning | gozen_diagnostics::Severity::Info => "warning",
    }
}

fn escape_message(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

pub fn report(diagnostics: &[gozen_diagnostics::Diagnostic], _summary: ReportSummary) {
    for d in diagnostics {
        let cmd = severity_cmd(d.severity);
        let file = d.file_path.as_deref().unwrap_or("");
        let line = d.span.start_row + 1;
        let col = d.span.start_col + 1;
        let end_col = d.span.end_col + 1;
        let title = d.rule_id.as_deref().unwrap_or("lint").replace('/', "-");
        let message = escape_message(&d.message);
        println!(
            "::{cmd} file={file},line={line},col={col},endColumn={end_col},title={title}::{message}"
        );
    }
}
