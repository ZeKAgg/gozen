pub mod ast;
mod gdscript;
mod gdshader;

pub use ast::{call_name, first_identifier_child, node_text, span_from_node, walk_tree};
pub use gdscript::GDScriptParser;
pub use gdshader::GDShaderParser;
pub use gozen_diagnostics::Span;
pub use tree_sitter::{Node, Tree, TreeCursor};
