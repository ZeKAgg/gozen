use gozen_diagnostics::Diagnostic;
use gozen_parser::Tree;

use crate::rule::RuleMetadata;

/// A lint rule for GDShader files.
pub trait ShaderRule: Send + Sync {
    fn metadata(&self) -> &RuleMetadata;
    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic>;
}
