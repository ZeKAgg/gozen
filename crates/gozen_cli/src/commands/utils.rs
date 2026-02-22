use gozen_diagnostics::Diagnostic;

/// Compute a content hash for a diagnostic, used by baseline and CI to match diagnostics.
pub fn content_hash_for_diagnostic(d: &Diagnostic) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    d.file_path.hash(&mut h);
    d.rule_id.hash(&mut h);
    d.message.hash(&mut h);
    d.span.start_row.hash(&mut h);
    d.span.start_col.hash(&mut h);
    format!("{:016x}", h.finish())
}
