use gozen_parser::{node_text, Node};

const CYCLOMATIC_BASE: usize = 1;

pub fn compute_cyclomatic_for_function(function_node: Node, source: &str) -> usize {
    let mut score = CYCLOMATIC_BASE;
    let target = function_body_node(function_node).unwrap_or(function_node);
    score += cyclomatic_visit(target, source);
    score
}

pub fn compute_cognitive_for_function(function_node: Node, source: &str) -> usize {
    let target = function_body_node(function_node).unwrap_or(function_node);
    cognitive_visit(target, source, 0)
}

fn function_body_node(function_node: Node) -> Option<Node> {
    for i in 0..function_node.child_count() {
        let child = function_node.child(i)?;
        if crate::rules::is_block_node(child.kind()) {
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
    } else if is_match_like(kind) {
        score += non_default_match_branches(node, source);
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
    } else if is_match_like(kind) {
        let branches = non_default_match_branches(node, source);
        score += branches.saturating_mul(1 + nesting);
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
    matches!(
        kind,
        "if_statement"
            | "elif_clause"
            | "elif"
            | "for_statement"
            | "for_in_statement"
            | "while_statement"
            | "for"
            | "while"
            | "if"
    )
}

fn is_ternary(kind: &str) -> bool {
    matches!(kind, "conditional_expression" | "ternary_expression")
}

fn is_match_like(kind: &str) -> bool {
    matches!(kind, "match_statement" | "switch_statement" | "switch")
}

fn is_flow_interrupt(kind: &str) -> bool {
    matches!(kind, "break_statement" | "continue_statement")
}

fn non_default_match_branches(node: Node, source: &str) -> usize {
    let mut branches = 0;
    for i in 0..node.child_count() {
        let Some(child) = node.child(i) else {
            continue;
        };
        if !child.is_named() {
            continue;
        }
        let kind = child.kind();
        if !(kind.contains("case")
            || kind.contains("pattern")
            || kind.contains("arm")
            || kind == "case")
        {
            continue;
        }
        let text = node_text(child, source).trim_start();
        let is_default = text.starts_with('_')
            || text.starts_with("default")
            || text.starts_with("else")
            || text.starts_with("_:");
        if !is_default {
            branches += 1;
        }
    }
    branches
}

fn boolean_operators_in_condition(node: Node, source: &str) -> usize {
    let text = node_text(node, source);
    count_operator_occurrences(text, " and ")
        + count_operator_occurrences(text, " or ")
        + count_operator_occurrences(text, "&&")
        + count_operator_occurrences(text, "||")
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
    use gozen_parser::{node_text, GDScriptParser};

    fn scores_for_function(source: &str, name: &str) -> (usize, usize) {
        let mut parser = GDScriptParser::new();
        let tree = parser.parse(source).expect("source parses");
        let root = tree.root_node();
        for i in 0..root.child_count() {
            if let Some(node) = root.child(i) {
                if node.kind() == "function_definition" && node_text(node, source).contains(name) {
                    return (
                        compute_cyclomatic_for_function(node, source),
                        compute_cognitive_for_function(node, source),
                    );
                }
            }
        }
        panic!("function definition exists");
    }

    #[test]
    fn flat_function_scores() {
        let source = "func a():\n\tvar x = 1\n\treturn x\n";
        let (cyc, cog) = scores_for_function(source, "a");
        assert_eq!(cyc, 1);
        assert_eq!(cog, 0);
    }

    #[test]
    fn if_and_nested_flow_scores() {
        let source = r#"func b():
	if x and y and z:
		while ok:
			if deep:
				break
"#;
        let (cyc, cog) = scores_for_function(source, "b");
        assert!(cyc >= 5, "expected cyclomatic >= 5, got {}", cyc);
        assert!(cog >= 8, "expected cognitive >= 8, got {}", cog);
    }
}
