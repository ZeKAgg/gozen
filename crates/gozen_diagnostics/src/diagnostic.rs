use crate::{Severity, Span};

#[derive(Debug, Clone)]
pub struct Note {
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct TextEdit {
    pub span: Span,
    pub new_text: String,
}

#[derive(Debug, Clone)]
pub struct Fix {
    pub description: String,
    pub is_safe: bool,
    pub changes: Vec<TextEdit>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub file_path: Option<String>,
    pub rule_id: Option<String>,
    pub span: Span,
    pub notes: Vec<Note>,
    pub fix: Option<Fix>,
}
