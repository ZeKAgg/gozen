use std::collections::HashSet;

use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct DuplicateDictionaryKey;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/duplicateDictionaryKey",
    name: "duplicateDictionaryKey",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "Duplicate keys in dictionary literals.",
    explanation: "Duplicate dictionary keys cause the later value to silently overwrite the earlier one. This is almost always a copy-paste bug.",
};

impl Rule for DuplicateDictionaryKey {
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
            let k = node.kind();
            if k == "dictionary" || k == "dictionary_literal" {
                let mut seen_keys: HashSet<String> = HashSet::new();
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        let ck = child.kind();
                        if ck == "pair" || ck == "dictionary_entry" || ck == "key_value_pair" {
                            // The key is typically the first named child
                            if let Some(key_node) = child.child(0) {
                                if key_node.is_named() {
                                    let key_text = node_text(key_node, src).to_string();
                                    // Normalize key: strip surrounding quotes for string keys,
                                    // but only if the key starts and ends with matching quotes
                                    let normalized = if (key_text.starts_with('"')
                                        && key_text.ends_with('"'))
                                        || (key_text.starts_with('\'') && key_text.ends_with('\''))
                                    {
                                        key_text[1..key_text.len() - 1].to_string()
                                    } else {
                                        key_text.clone()
                                    };
                                    if !seen_keys.insert(normalized) {
                                        diags.push(Diagnostic {
                                            severity: Severity::Error,
                                            message: format!(
                                                "Duplicate dictionary key: {}.",
                                                key_text
                                            ),
                                            file_path: None,
                                            rule_id: None,
                                            span: span_from_node(key_node),
                                            notes: vec![],
                                            fix: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        diags
    }
}
