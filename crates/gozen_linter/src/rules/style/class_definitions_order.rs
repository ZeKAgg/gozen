use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{node_text, span_from_node, Tree};

use crate::rule::{Rule, RuleMetadata};

/// Category indices for GDScript class member ordering per Godot style guide.
/// Lower index = should appear earlier in the file.
const fn category_priority(cat: Category) -> usize {
    match cat {
        Category::Tool => 0,
        Category::ClassName => 1,
        Category::Extends => 2,
        Category::Signal => 3,
        Category::Enum => 4,
        Category::Constant => 5,
        Category::ExportVar => 6,
        Category::PublicVar => 7,
        Category::PrivateVar => 8,
        Category::OnreadyVar => 9,
        Category::LifecycleMethod => 10,
        Category::PublicMethod => 11,
        Category::PrivateMethod => 12,
        Category::InnerClass => 13,
    }
}

fn category_label(cat: Category) -> &'static str {
    match cat {
        Category::Tool => "@tool",
        Category::ClassName => "class_name",
        Category::Extends => "extends",
        Category::Signal => "signal declarations",
        Category::Enum => "enum declarations",
        Category::Constant => "constants",
        Category::ExportVar => "@export variables",
        Category::PublicVar => "public variables",
        Category::PrivateVar => "private variables",
        Category::OnreadyVar => "@onready variables",
        Category::LifecycleMethod => "lifecycle methods",
        Category::PublicMethod => "public methods",
        Category::PrivateMethod => "private methods",
        Category::InnerClass => "inner classes",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Tool,
    ClassName,
    Extends,
    Signal,
    Enum,
    Constant,
    ExportVar,
    PublicVar,
    PrivateVar,
    OnreadyVar,
    LifecycleMethod,
    PublicMethod,
    PrivateMethod,
    InnerClass,
}

const LIFECYCLE_METHODS: &[&str] = &[
    "_init",
    "_enter_tree",
    "_exit_tree",
    "_ready",
    "_process",
    "_physics_process",
    "_input",
    "_unhandled_input",
    "_unhandled_key_input",
    "_draw",
    "_integrate_forces",
    "_notification",
    "_get_configuration_warnings",
    "_get_property_list",
    "_set",
    "_get",
];

fn classify_node(node: gozen_parser::Node, source: &str) -> Option<Category> {
    let kind = node.kind();
    match kind {
        "tool_statement" => Some(Category::Tool),
        "class_name_statement" => Some(Category::ClassName),
        "extends_statement" => Some(Category::Extends),
        "signal_statement" | "signal_declaration" => Some(Category::Signal),
        "enum_definition" | "enum_statement" => Some(Category::Enum),
        "const_statement" | "constant_definition" => Some(Category::Constant),
        "class_definition" => Some(Category::InnerClass),
        "variable_statement" => {
            // Check the raw text of the statement to determine subcategory
            let text = node_text(node, source);
            let has_onready = text.contains("@onready");
            let has_export = text.contains("@export");

            if has_onready {
                Some(Category::OnreadyVar)
            } else if has_export {
                Some(Category::ExportVar)
            } else {
                // Check if name starts with underscore (private)
                let name = extract_var_name(node, source);
                if name.starts_with('_') {
                    Some(Category::PrivateVar)
                } else {
                    Some(Category::PublicVar)
                }
            }
        }
        "function_definition" => {
            let name = extract_func_name(node, source);
            if LIFECYCLE_METHODS.contains(&name) {
                Some(Category::LifecycleMethod)
            } else if name.starts_with('_') {
                Some(Category::PrivateMethod)
            } else {
                Some(Category::PublicMethod)
            }
        }
        // Decorated definitions: @export var, @onready var, @tool, etc.
        "decorated_definition" => {
            // Look at the child nodes: first the decorator(s), then the actual definition
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    let ck = child.kind();
                    if ck == "variable_statement" {
                        let text = node_text(node, source);
                        if text.contains("@onready") {
                            return Some(Category::OnreadyVar);
                        }
                        if text.contains("@export") {
                            return Some(Category::ExportVar);
                        }
                        let name = extract_var_name(child, source);
                        if name.starts_with('_') {
                            return Some(Category::PrivateVar);
                        }
                        return Some(Category::PublicVar);
                    } else if ck == "function_definition" {
                        let name = extract_func_name(child, source);
                        if LIFECYCLE_METHODS.contains(&name) {
                            return Some(Category::LifecycleMethod);
                        }
                        if name.starts_with('_') {
                            return Some(Category::PrivateMethod);
                        }
                        return Some(Category::PublicMethod);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_var_name<'a>(node: gozen_parser::Node<'a>, source: &'a str) -> &'a str {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return node_text(child, source);
            }
        }
    }
    ""
}

fn extract_func_name<'a>(node: gozen_parser::Node<'a>, source: &'a str) -> &'a str {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return node_text(child, source);
            }
        }
    }
    ""
}

pub struct ClassDefinitionsOrder;

const METADATA: RuleMetadata = RuleMetadata {
    id: "style/classDefinitionsOrder",
    name: "classDefinitionsOrder",
    group: "style",
    default_severity: Severity::Warning,
    has_fix: false,
    description: "Class members should follow the Godot GDScript style guide ordering.",
    explanation: "The recommended order is: @tool, class_name, extends, signals, enums, constants, @export variables, public variables, private variables, @onready variables, lifecycle methods (_init, _ready, etc.), public methods, private methods, inner classes.",
};

impl Rule for ClassDefinitionsOrder {
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
        let mut max_priority: usize = 0;
        let mut max_category: Option<Category> = None;

        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if !child.is_named() {
                    continue;
                }
                if let Some(cat) = classify_node(child, source) {
                    let priority = category_priority(cat);
                    if priority < max_priority {
                        let after_label = max_category
                            .map(category_label)
                            .unwrap_or("previous declarations");
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "{} should appear before {}.",
                                category_label(cat),
                                after_label,
                            ),
                            file_path: None,
                            rule_id: None,
                            span: span_from_node(child),
                            notes: vec![],
                            fix: None,
                        });
                    } else {
                        max_priority = priority;
                        max_category = Some(cat);
                    }
                }
            }
        }
        diags
    }
}
