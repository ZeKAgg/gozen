use gozen_parser::{node_text, Node};

const CYCLOMATIC_BASE: usize = 1;

pub fn compute_cyclomatic_for_function(function_node: Node, source: &str) -> usize {
    let mut score = CYCLOMATIC_BASE;
    let body = function_body_node(function_node).unwrap_or(function_node);
    score += cyclomatic_visit(body, source);
    score
}

pub fn compute_cognitive_for_function(function_node: Node, source: &str) -> usize {
    let body = function_body_node(function_node).unwrap_or(function_node);
    cognitive_visit(body, source, 0)
}

fn function_body_node(function_node: Node) -> Option<Node> {
    for i in 0..function_node.child_count() {
        let child = function_node.child(i)?;
        if child.kind() == "block" || child.kind() == "body" {
            return Some(child);
        }
    }
    None
}

fn cyclomatic_visit(node: Node, source: &str) -> usize {
    let mut score = 0;
    let kind = node.kind();

    if is_simple_decision(kind) || is_ternary(kind) {
        score += 1;
        score += boolean_operators_in_condition(node, source);
    } else if is_switch_like(kind) {
        score += non_default_switch_cases(node, source);
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.is_named() {
                score += cyclomatic_visit(child, source);
            }
        }
    }
    score
}

fn cognitive_visit(node: Node, source: &str, nesting: usize) -> usize {
    let mut score = 0;
    let kind = node.kind();
    let mut child_nesting = nesting;

    if is_simple_decision(kind) || is_ternary(kind) {
        score += 1 + nesting;
        score += boolean_operators_in_condition(node, source);
        child_nesting += 1;
    } else if is_switch_like(kind) {
        let cases = non_default_switch_cases(node, source);
        score += cases.saturating_mul(1 + nesting);
        child_nesting += 1;
    } else if is_flow_interrupt(kind) && nesting > 0 {
        score += 1;
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.is_named() {
                score += cognitive_visit(child, source, child_nesting);
            }
        }
    }
    score
}

fn is_simple_decision(kind: &str) -> bool {
    matches!(kind, "if_statement" | "for_statement" | "while_statement")
}

fn is_ternary(kind: &str) -> bool {
    matches!(kind, "conditional_expression" | "ternary_expression")
}

fn is_switch_like(kind: &str) -> bool {
    matches!(kind, "switch_statement" | "switch")
}

fn is_flow_interrupt(kind: &str) -> bool {
    matches!(kind, "break_statement" | "continue_statement")
}

fn non_default_switch_cases(node: Node, source: &str) -> usize {
    let mut count = 0;
    for i in 0..node.child_count() {
        let Some(child) = node.child(i) else {
            continue;
        };
        if !child.is_named() {
            continue;
        }
        let kind = child.kind();
        if !(kind == "case" || kind.contains("case")) {
            continue;
        }
        let text = node_text(child, source).trim_start();
        let is_default = text.starts_with("default");
        if !is_default {
            count += 1;
        }
    }
    count
}

fn boolean_operators_in_condition(node: Node, source: &str) -> usize {
    let text = node_text(node, source);
    count_operator_occurrences(text, "&&")
        + count_operator_occurrences(text, "||")
        + count_operator_occurrences(text, " and ")
        + count_operator_occurrences(text, " or ")
}

fn count_operator_occurrences(input: &str, needle: &str) -> usize {
    if needle.trim().is_empty() {
        return 0;
    }
    input.match_indices(needle).count()
}

#[cfg(test)]
mod tests {
    use super::{compute_cognitive_for_function, compute_cyclomatic_for_function};
    use gozen_parser::{node_text, GDShaderParser};

    fn scores_for_function(source: &str, name: &str) -> (usize, usize) {
        let mut parser = GDShaderParser::new();
        let tree = parser.parse(source).expect("source parses");
        let root = tree.root_node();
        for i in 0..root.child_count() {
            if let Some(node) = root.child(i) {
                if node.kind() == "function_declaration" {
                    let text = node_text(node, source);
                    if text.contains(name) {
                        return (
                            compute_cyclomatic_for_function(node, source),
                            compute_cognitive_for_function(node, source),
                        );
                    }
                }
            }
        }
        panic!("function exists");
    }

    #[test]
    fn flat_function_scores() {
        let source = r#"
shader_type spatial;
void helper() {
    float x = 1.0;
}
"#;
        let (cyc, cog) = scores_for_function(source, "helper");
        assert_eq!(cyc, 1);
        assert_eq!(cog, 0);
    }

    #[test]
    fn nested_flow_scores() {
        let source = r#"
shader_type spatial;
void helper() {
    if (a && b && c) {
        while (ok) {
            if (deep) {
                break;
            }
        }
    }
}
"#;
        let (cyc, cog) = scores_for_function(source, "helper");
        assert!(cyc >= 5, "expected cyclomatic >= 5, got {}", cyc);
        assert!(cog >= 8, "expected cognitive >= 8, got {}", cog);
    }
}
