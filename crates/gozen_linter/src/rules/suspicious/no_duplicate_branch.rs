use std::collections::HashSet;

use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoDuplicateBranch;

const METADATA: RuleMetadata = RuleMetadata {
    id: "suspicious/noDuplicateBranch",
    name: "noDuplicateBranch",
    group: "suspicious",
    default_severity: Severity::Error,
    has_fix: false,
    description: "if/elif branches with identical bodies.",
    explanation: "Duplicate branches are likely a copy-paste error. Merge conditions.",
};

impl Rule for NoDuplicateBranch {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        tree: &Tree,
        source: &str,
        _context: Option<&crate::context::LintContext>,
    ) -> Vec<Diagnostic> {
        let root = tree.root_node();
        let mut diags = Vec::new();
        walk_tree(root, source, |node, src| {
            if node.kind() == "if_statement" {
                let bodies: Vec<String> = collect_branch_bodies(node, src);
                // Use a HashSet for O(n) duplicate detection instead of O(n^2) nested loops
                let mut seen = HashSet::new();
                for body in &bodies {
                    let normalized = normalize_ws(body);
                    if !seen.insert(normalized) {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            message: "Duplicate branch body.".to_string(),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(node),
                            notes: vec![],
                            fix: None,
                        });
                        return;
                    }
                }
            }
        });
        diags
    }
}

fn collect_branch_bodies(node: Node, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..node.child_count() {
        let c = match node.child(i) {
            Some(c) => c,
            None => continue,
        };
        if c.is_named() && crate::rules::is_block_node(c.kind()) {
            out.push(node_text(c, source).to_string());
        }
    }
    out
}

/// Normalize whitespace for branch body comparison.
/// Trims each line and collapses blank lines, but preserves intra-line spacing
/// so that "x = 1" and "x=1" are not falsely considered identical.
fn normalize_ws(s: &str) -> String {
    s.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
