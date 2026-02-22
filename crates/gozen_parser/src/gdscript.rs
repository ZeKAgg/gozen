use tree_sitter::{Parser, Tree};

pub struct GDScriptParser {
    parser: Parser,
}

impl GDScriptParser {
    pub fn new() -> Self {
        Self::try_new().unwrap_or_else(|_| Self {
            parser: Parser::new(),
        })
    }

    pub fn try_new() -> std::result::Result<Self, tree_sitter::LanguageError> {
        let mut parser = Parser::new();
        let language = tree_sitter_gdscript::LANGUAGE;
        parser.set_language(&language.into())?;
        Ok(Self { parser })
    }

    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }
}

impl Default for GDScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_script() {
        let mut parser = GDScriptParser::new();
        let source = r#"
extends Node

var health: int = 100

func _ready():
    print("hello")
"#;
        let tree = parser.parse(source).expect("Failed to parse");
        let root = tree.root_node();
        assert!(!root.has_error(), "Parse tree has errors");
    }

    #[test]
    fn test_parse_with_errors_recovers() {
        let mut parser = GDScriptParser::new();
        let source = "func foo(\n    print('hi')";
        let tree = parser.parse(source).expect("Failed to parse");
        let root = tree.root_node();
        assert!(root.has_error());
        assert!(root.child_count() > 0);
    }
}
