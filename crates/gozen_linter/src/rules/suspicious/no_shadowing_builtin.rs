use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{first_identifier_child, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct NoShadowingBuiltin;

const METADATA: RuleMetadata = RuleMetadata {
    id: "suspicious/noShadowingBuiltin",
    name: "noShadowingBuiltin",
    group: "suspicious",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Variable names that shadow GDScript built-in functions or constants.",
    explanation: "Shadowing built-in names like print, sin, PI, etc. prevents access to the original and is almost always a mistake.",
};

/// GDScript built-in functions that should not be shadowed.
const BUILTIN_FUNCTIONS: &[&str] = &[
    "print",
    "print_rich",
    "print_verbose",
    "push_error",
    "push_warning",
    "printerr",
    "printraw",
    "prints",
    "printt",
    "abs",
    "absf",
    "absi",
    "acos",
    "asin",
    "atan",
    "atan2",
    "ceil",
    "ceilf",
    "ceili",
    "clamp",
    "clampf",
    "clampi",
    "cos",
    "deg_to_rad",
    "ease",
    "exp",
    "floor",
    "floorf",
    "floori",
    "fmod",
    "fposmod",
    "hash",
    "is_equal_approx",
    "is_finite",
    "is_inf",
    "is_nan",
    "is_same",
    "is_zero_approx",
    "lerp",
    "lerpf",
    "log",
    "max",
    "maxf",
    "maxi",
    "min",
    "minf",
    "mini",
    "move_toward",
    "nearest_po2",
    "pingpong",
    "posmod",
    "pow",
    "rad_to_deg",
    "randf",
    "randf_range",
    "randi",
    "randi_range",
    "randomize",
    "remap",
    "round",
    "roundf",
    "roundi",
    "seed",
    "sign",
    "signf",
    "signi",
    "sin",
    "snapped",
    "snappedf",
    "snappedi",
    "sqrt",
    "str",
    "tan",
    "typeof",
    "weakref",
    "wrap",
    "wrapf",
    "wrapi",
    "range",
    "load",
    "preload",
    "len",
    "type_string",
    "var_to_str",
    "str_to_var",
    "var_to_bytes",
    "bytes_to_var",
    "is_instance_of",
    "is_instance_valid",
];

/// GDScript built-in constants that should not be shadowed.
const BUILTIN_CONSTANTS: &[&str] = &["PI", "TAU", "INF", "NAN"];

/// Common built-in type names that should not be shadowed.
const BUILTIN_TYPES: &[&str] = &[
    "Vector2",
    "Vector2i",
    "Vector3",
    "Vector3i",
    "Vector4",
    "Vector4i",
    "Rect2",
    "Rect2i",
    "Transform2D",
    "Transform3D",
    "Color",
    "Plane",
    "Quaternion",
    "AABB",
    "Basis",
    "RID",
    "Array",
    "Dictionary",
    "String",
    "StringName",
    "NodePath",
    "Object",
    "Node",
    "Resource",
    "PackedScene",
];

impl Rule for NoShadowingBuiltin {
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
            if k != "variable_statement" && k != "const_statement" && k != "constant_definition" {
                return;
            }
            if let Some(name_node) = first_identifier_child(node) {
                let name = node_text(name_node, src);
                if BUILTIN_FUNCTIONS.contains(&name) {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Variable \"{}\" shadows the built-in function {}().",
                            name, name
                        ),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(name_node),
                        notes: vec![],
                        fix: None,
                    });
                } else if BUILTIN_CONSTANTS.contains(&name) {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Variable \"{}\" shadows the built-in constant {}.",
                            name, name
                        ),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(name_node),
                        notes: vec![],
                        fix: None,
                    });
                } else if BUILTIN_TYPES.contains(&name) {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Variable \"{}\" shadows the built-in type {}.",
                            name, name
                        ),
                        file_path: None,
                        rule_id: None,
                        span: span_from_node(name_node),
                        notes: vec![],
                        fix: None,
                    });
                }
            }
        });
        diags
    }
}
