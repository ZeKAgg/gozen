use tree_sitter::Node;

use crate::Span;

/// Build a Span from a tree-sitter Node (avoids orphan rule; both Node and Span are external).
pub fn span_from_node(node: Node) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    Span {
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_row: start.row,
        start_col: start.column,
        end_row: end.row,
        end_col: end.column,
    }
}

/// Extract text for a node from source.
/// Returns `""` if byte offsets are out of bounds (defensive against malformed ASTs).
pub fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    source.get(node.start_byte()..node.end_byte()).unwrap_or("")
}

/// Depth-first walk of the tree, calling `callback` for each node.
pub fn walk_tree<F>(root: Node, source: &str, mut callback: F)
where
    F: FnMut(Node, &str),
{
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        callback(node, source);

        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// Find the first named child with kind "identifier".
pub fn first_identifier_child(node: Node) -> Option<Node> {
    for i in 0..node.child_count() {
        let c = node.child(i)?;
        if c.is_named() && c.kind() == "identifier" {
            return Some(c);
        }
    }
    None
}

/// Extract the call name from a call_expression or call node.
/// Returns the text of the first "identifier" or "name" child.
pub fn call_name<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) {
            if c.kind() == "identifier" || c.kind() == "name" {
                return node_text(c, source);
            }
        }
    }
    ""
}
