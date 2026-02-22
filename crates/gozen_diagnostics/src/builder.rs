use crate::{Diagnostic, Fix, Note, Severity, Span, TextEdit};

/// Builder for constructing `Diagnostic` instances with less boilerplate.
///
/// # Example
///
/// ```ignore
/// use gozen_diagnostics::{DiagnosticBuilder, Severity, Span};
///
/// let diag = DiagnosticBuilder::new(Severity::Warning, "Unused variable")
///     .span(my_span)
///     .note("Consider prefixing with _ to suppress this warning")
///     .safe_fix("Remove unused variable", vec![TextEdit { span: my_span, new_text: String::new() }])
///     .build();
/// ```
pub struct DiagnosticBuilder {
    severity: Severity,
    message: String,
    span: Span,
    file_path: Option<String>,
    rule_id: Option<String>,
    notes: Vec<Note>,
    fix: Option<Fix>,
}

impl DiagnosticBuilder {
    /// Create a new builder with the required severity and message.
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            span: Span {
                start_byte: 0,
                end_byte: 0,
                start_row: 0,
                start_col: 0,
                end_row: 0,
                end_col: 0,
            },
            file_path: None,
            rule_id: None,
            notes: Vec::new(),
            fix: None,
        }
    }

    /// Create a warning diagnostic.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    /// Create an error diagnostic.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    /// Set the source span for this diagnostic.
    pub fn span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Set the file path.
    pub fn file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Set the rule ID.
    pub fn rule_id(mut self, id: impl Into<String>) -> Self {
        self.rule_id = Some(id.into());
        self
    }

    /// Add a note to this diagnostic.
    pub fn note(mut self, message: impl Into<String>) -> Self {
        self.notes.push(Note {
            message: message.into(),
            span: None,
        });
        self
    }

    /// Add a note with a source span.
    pub fn note_with_span(mut self, message: impl Into<String>, span: Span) -> Self {
        self.notes.push(Note {
            message: message.into(),
            span: Some(span),
        });
        self
    }

    /// Add a safe auto-fix (can be applied without user confirmation).
    pub fn safe_fix(mut self, description: impl Into<String>, changes: Vec<TextEdit>) -> Self {
        self.fix = Some(Fix {
            description: description.into(),
            is_safe: true,
            changes,
        });
        self
    }

    /// Add an unsafe fix (requires user confirmation).
    pub fn unsafe_fix(mut self, description: impl Into<String>, changes: Vec<TextEdit>) -> Self {
        self.fix = Some(Fix {
            description: description.into(),
            is_safe: false,
            changes,
        });
        self
    }

    /// Build the final `Diagnostic`.
    pub fn build(self) -> Diagnostic {
        Diagnostic {
            severity: self.severity,
            message: self.message,
            file_path: self.file_path,
            rule_id: self.rule_id,
            span: self.span,
            notes: self.notes,
            fix: self.fix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let span = Span {
            start_byte: 0,
            end_byte: 10,
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 10,
        };
        let diag = DiagnosticBuilder::warning("test warning")
            .span(span)
            .build();

        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.message, "test warning");
        assert_eq!(diag.span.end_byte, 10);
        assert!(diag.fix.is_none());
        assert!(diag.notes.is_empty());
    }

    #[test]
    fn test_builder_with_fix() {
        let span = Span {
            start_byte: 0,
            end_byte: 5,
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 5,
        };
        let diag = DiagnosticBuilder::error("unused variable")
            .span(span)
            .note("Consider removing this variable")
            .safe_fix(
                "Remove unused variable",
                vec![TextEdit {
                    span,
                    new_text: String::new(),
                }],
            )
            .build();

        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.notes.len(), 1);
        assert!(diag.fix.is_some());
        let fix = diag.fix.unwrap();
        assert!(fix.is_safe);
        assert_eq!(fix.changes.len(), 1);
    }
}
