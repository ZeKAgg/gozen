use serde::Serialize;

use super::ReportSummary;

#[derive(Serialize)]
struct JsonSpan {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

#[derive(Serialize)]
struct JsonDiagnostic {
    file: Option<String>,
    rule: Option<String>,
    severity: String,
    message: String,
    span: JsonSpan,
}

#[derive(Serialize)]
struct JsonSummary {
    errors: usize,
    warnings: usize,
    #[serde(rename = "filesChecked")]
    files_checked: usize,
    #[serde(rename = "durationMs")]
    duration_ms: u64,
    #[serde(rename = "formatFailures", skip_serializing_if = "is_zero")]
    format_failures: usize,
}

fn is_zero(x: &usize) -> bool {
    *x == 0
}

#[derive(Serialize)]
struct JsonOutput {
    diagnostics: Vec<JsonDiagnostic>,
    summary: JsonSummary,
}

pub fn report(diagnostics: &[gozen_diagnostics::Diagnostic], summary: ReportSummary) {
    let diagnostics: Vec<JsonDiagnostic> = diagnostics
        .iter()
        .map(|d| {
            let severity = match d.severity {
                gozen_diagnostics::Severity::Error => "error",
                gozen_diagnostics::Severity::Warning | gozen_diagnostics::Severity::Info => {
                    "warning"
                }
            };
            JsonDiagnostic {
                file: d.file_path.clone(),
                rule: d.rule_id.clone(),
                severity: severity.to_string(),
                message: d.message.clone(),
                span: JsonSpan {
                    start_line: d.span.start_row + 1,
                    start_column: d.span.start_col + 1,
                    end_line: d.span.end_row + 1,
                    end_column: d.span.end_col + 1,
                },
            }
        })
        .collect();
    let output = JsonOutput {
        diagnostics,
        summary: JsonSummary {
            errors: summary.errors,
            warnings: summary.warnings,
            files_checked: summary.files_checked,
            duration_ms: summary.duration_ms,
            format_failures: summary.format_failures,
        },
    };
    match serde_json::to_string_pretty(&output) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("Error: failed to serialize JSON output: {}", e),
    }
}
