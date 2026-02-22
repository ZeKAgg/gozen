pub mod builder;
pub mod diagnostic;
pub mod render;
pub mod severity;
pub mod span;

pub use builder::DiagnosticBuilder;
pub use diagnostic::{Diagnostic, Fix, Note, TextEdit};
pub use render::render_diagnostic;
pub use severity::Severity;
pub use span::Span;
