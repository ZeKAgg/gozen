use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::Tree;
use gozen_project::ProjectGraph;

use crate::context::LintContext;

pub struct RuleMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub default_severity: Severity,
    pub has_fix: bool,
    pub description: &'static str,
    pub explanation: &'static str,
}

pub trait Rule: Send + Sync {
    fn metadata(&self) -> &RuleMetadata;
    fn check(&self, tree: &Tree, source: &str, context: Option<&LintContext>) -> Vec<Diagnostic>;
}

/// A rule that needs the project graph in addition to the AST.
pub trait ProjectRule: Send + Sync {
    fn metadata(&self) -> &RuleMetadata;
    fn check(
        &self,
        tree: &Tree,
        source: &str,
        graph: &ProjectGraph,
        script_path: &str,
    ) -> Vec<Diagnostic>;
}
