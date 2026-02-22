use gozen_diagnostics::{Diagnostic, Note, Severity};
use gozen_parser::{call_name, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoDeprecatedApi;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/noDeprecatedApi",
    name: "noDeprecatedApi",
    group: "correctness",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Usage of Godot 3.x API patterns that are changed or removed in Godot 4.x.",
    explanation: "Godot 4 changed many APIs. Using old patterns will cause runtime errors or unexpected behavior.",
};

/// Known Godot 3 class names that were renamed in Godot 4.
const RENAMED_CLASSES: &[(&str, &str)] = &[
    ("KinematicBody2D", "CharacterBody2D"),
    ("KinematicBody", "CharacterBody3D"),
    ("StaticBody", "StaticBody3D"),
    ("Spatial", "Node3D"),
    ("GIProbe", "VoxelGI"),
    ("BakedLightmap", "LightmapGI"),
    ("VisibilityNotifier2D", "VisibleOnScreenNotifier2D"),
    ("VisibilityNotifier3D", "VisibleOnScreenNotifier3D"),
    ("VisibilityEnabler2D", "VisibleOnScreenEnabler2D"),
    ("VisibilityEnabler3D", "VisibleOnScreenEnabler3D"),
    ("Navigation2DServer", "NavigationServer2D"),
    ("NavigationServer", "NavigationServer3D"),
    ("OpenSimplexNoise", "FastNoiseLite"),
    ("StreamPeerSSL", "StreamPeerTLS"),
    ("YSort", "Node2D"), // YSort removed, use CanvasItem.y_sort_enabled
    ("Position2D", "Marker2D"),
    ("Position3D", "Marker3D"),
];

/// Known Godot 3 method patterns that changed.
const DEPRECATED_METHODS: &[(&str, &str)] = &[
    ("instance", "instantiate"),
    ("set_cell_size", "Use TileMap API changes"),
    ("get_color", "get_theme_color"),
    ("get_font", "get_theme_font"),
    ("get_icon", "get_theme_icon"),
    ("get_stylebox", "get_theme_stylebox"),
    ("get_constant", "get_theme_constant"),
];

impl Rule for NoDeprecatedApi {
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

            // Check extends statements for renamed classes
            if k == "extends_statement" || k == "class_definition" {
                let text = node_text(node, src);
                for (old, new) in RENAMED_CLASSES {
                    if contains_word(text, old) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("\"{}\" was renamed to \"{}\" in Godot 4.", old, new),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(node),
                            notes: vec![Note {
                                message: format!("Replace with: {}", new),
                                span: None,
                            }],
                            fix: None,
                        });
                    }
                }
            }

            // Check identifiers for renamed classes used as types.
            // Only flag identifiers in type contexts (extends, type hints, .new() calls)
            // to avoid false positives on variable/function names.
            if k == "identifier" {
                // Check if the parent is a type-related context
                let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
                let is_type_context = parent_kind == "extends_statement"
                    || parent_kind == "class_definition"
                    || parent_kind == "type"
                    || parent_kind == "type_hint"
                    || parent_kind == "typed_parameter"
                    || parent_kind == "return_type"
                    || parent_kind == "cast_expression"
                    || parent_kind == "is_expression";
                if !is_type_context {
                    return; // Skip non-type identifiers
                }
                let text = node_text(node, src);
                for (old, new) in RENAMED_CLASSES {
                    if text == *old {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("\"{}\" was renamed to \"{}\" in Godot 4.", old, new),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(node),
                            notes: vec![Note {
                                message: format!("Replace with: {}", new),
                                span: None,
                            }],
                            fix: None,
                        });
                    }
                }
            }

            // Check for deprecated method calls
            if k == "call_expression" || k == "call" {
                let name = call_name(node, src);
                for (old, new) in DEPRECATED_METHODS {
                    if name == *old {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "\"{}\" is deprecated in Godot 4. Use \"{}\" instead.",
                                old, new
                            ),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(node),
                            notes: vec![],
                            fix: None,
                        });
                    }
                }
            }
        });
        diags
    }
}

/// Check if `haystack` contains `needle` as a whole word
/// (i.e. not as a substring of a longer identifier).
fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let abs_pos = start + pos;
        let end_pos = abs_pos + needle.len();
        let before_ok = abs_pos == 0
            || (!haystack.as_bytes()[abs_pos - 1].is_ascii_alphanumeric()
                && haystack.as_bytes()[abs_pos - 1] != b'_');
        let after_ok = end_pos >= haystack.len()
            || (!haystack.as_bytes()[end_pos].is_ascii_alphanumeric()
                && haystack.as_bytes()[end_pos] != b'_');
        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + 1;
    }
    false
}
