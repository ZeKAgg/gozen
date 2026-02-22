use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{node_text, span_from_node, walk_tree, Node, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoLoopAllocation;

const METADATA: RuleMetadata = RuleMetadata {
    id: "performance/noLoopAllocation",
    name: "noLoopAllocation",
    group: "performance",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Object allocation (.new()) inside a loop.",
    explanation: "Calling .new() inside a loop creates a new object each iteration, which can cause GC pressure and stutter. Consider reusing objects via an object pool or moving the allocation outside the loop.",
};

impl Rule for NoLoopAllocation {
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
        walk_tree(root, source, |node, _src| {
            let k = node.kind();
            if k == "for_statement" || k == "while_statement" || k == "for_in_statement" {
                check_loop_body(node, source, &mut diags);
            }
        });
        diags
    }
}

/// Value types in Godot that are stack-allocated and safe to create in loops.
const VALUE_TYPES: &[&str] = &[
    "Vector2",
    "Vector2i",
    "Vector3",
    "Vector3i",
    "Vector4",
    "Vector4i",
    "Rect2",
    "Rect2i",
    "Color",
    "Transform2D",
    "Transform3D",
    "Basis",
    "Quaternion",
    "AABB",
    "Plane",
    "Projection",
    "RID",
    "StringName",
    "NodePath",
];

fn check_loop_body(loop_node: Node, source: &str, diags: &mut Vec<Diagnostic>) {
    walk_tree(loop_node, source, |node, src| {
        if node.kind() != "call_expression" && node.kind() != "call" {
            return;
        }

        let text = node_text(node, src).trim();

        // Check for .new() calls like ClassName.new()
        if !text.contains(".new(") {
            return;
        }

        // Extract the class name before .new(
        if let Some(dot_new_pos) = text.find(".new(") {
            let class_part = text[..dot_new_pos].trim();
            // Get the last identifier (handle chained access like Foo.Bar.new())
            let class_name = class_part.rsplit('.').next().unwrap_or(class_part);

            // Skip value types — they're stack-allocated in Godot
            if VALUE_TYPES.contains(&class_name) {
                return;
            }

            diags.push(Diagnostic {
                severity: Severity::Warning,
                message: format!("Object allocation \"{}.new()\" inside a loop.", class_name),
                file_path: None,
                rule_id: None,
                span: span_from_node(node),
                notes: vec![Note {
                    message: "Move the allocation outside the loop or use an object pool."
                        .to_string(),
                    span: None,
                }],
                fix: None,
            });
        }
    });
}
