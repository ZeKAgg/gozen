use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::Tree;

use crate::rule::{Rule, RuleMetadata};

pub struct FileNaming;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/fileNaming",
    name: "fileNaming",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "GDScript filenames should be snake_case.",
    explanation: "The GDScript style guide recommends snake_case filenames. This avoids case-sensitivity issues when exporting from Windows to other platforms.",
};

impl Rule for FileNaming {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        _tree: &Tree,
        _source: &str,
        context: Option<&crate::context::LintContext>,
    ) -> Vec<Diagnostic> {
        // This rule checks the file path, which comes from context.
        // Since the file_path is set by the engine after check() returns,
        // we rely on the context's project_root to deduce the file name.
        // In practice, this rule works best as a text-based check on the file name.
        // We'll check using the source — if the file is being linted, its path
        // will be set. For now, we emit based on context.
        let _ = context;
        // Note: The actual filename check is done in a wrapper since
        // the Rule trait doesn't receive the file path. We implement the
        // detection logic here but it will need the engine to pass file_path.
        // For now, this is a no-op placeholder that documents intent.
        // The actual check is text-based in the lint pipeline.
        Vec::new()
    }
}

/// Standalone function to check a filename. Called by the engine or CLI
/// when the file path is available.
pub fn check_filename(file_path: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    // Extract just the filename without extension
    let file_name = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if file_name.is_empty() {
        return diags;
    }

    if !is_snake_case(file_name) {
        diags.push(Diagnostic {
            severity: Severity::Warning,
            message: format!("Filename \"{}\" should be snake_case.", file_name),
            file_path: Some(file_path.to_string()),
            rule_id: Some("style/fileNaming".to_string()),
            span: Span {
                start_byte: 0,
                end_byte: 0,
                start_row: 0,
                start_col: 0,
                end_row: 0,
                end_col: 0,
            },
            notes: vec![],
            fix: None,
        });
    }
    diags
}

fn is_snake_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars()
        .all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
}
