use crate::{Diagnostic, Severity};
use colored::Colorize;

/// Pretty-print a single diagnostic (Biome-style).
pub fn render_diagnostic(diag: &Diagnostic, source: Option<&str>) -> String {
    let header = diag.rule_id.as_deref().unwrap_or("diagnostic").to_string();
    let severity_str = match diag.severity {
        Severity::Error => "✖".red().to_string(),
        Severity::Warning => "✖".yellow().to_string(),
        Severity::Info => "✖".bright_blue().to_string(),
    };
    let mut out = format!(
        "{} {} ━━━━━━━━━━━━━━━━━━\n\n  {} {}",
        diag.file_path.as_deref().unwrap_or(""),
        header,
        severity_str,
        diag.message
    );
    if let Some(src) = source {
        let line = src.lines().nth(diag.span.start_row).unwrap_or("");
        out.push_str(&format!("\n\n  {} │ {}", diag.span.start_row + 1, line));
    }
    // Render notes if present
    for note in &diag.notes {
        out.push_str(&format!("\n  ℹ {}", note.message));
    }
    // Render fix suggestion if present
    if let Some(fix) = &diag.fix {
        let safety = if fix.is_safe {
            "Safe fix"
        } else {
            "Unsafe fix"
        };
        out.push_str(&format!("\n  ⚡ {}: {}", safety, fix.description));
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, Severity, Span};

    use super::render_diagnostic;

    #[test]
    fn test_render_diagnostic() {
        let diag = Diagnostic {
            severity: Severity::Warning,
            message: "Variable \"temp\" is declared but never used.".into(),
            file_path: Some("scripts/player.gd".into()),
            rule_id: Some("correctness/noUnusedVariables".into()),
            span: Span {
                start_byte: 20,
                end_byte: 24,
                start_row: 1,
                start_col: 8,
                end_row: 1,
                end_col: 12,
            },
            notes: vec![],
            fix: None,
        };
        let source = "func _ready():\n    var temp = 10\n    print(\"hello\")";
        let out = render_diagnostic(&diag, Some(source));
        assert!(out.contains("scripts/player.gd"));
        assert!(out.contains("correctness/noUnusedVariables"));
        assert!(out.contains("Variable \"temp\" is declared but never used."));
        assert!(out.contains("2 │")); // 1-based line for "var temp = 10"
        assert!(out.contains("var temp = 10"));
    }
}
