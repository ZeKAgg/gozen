use tree_sitter::{Parser, Tree};

pub struct GDShaderParser {
    parser: Parser,
}

impl GDShaderParser {
    pub fn new() -> Self {
        Self::try_new().unwrap_or_else(|_| Self {
            parser: Parser::new(),
        })
    }

    pub fn try_new() -> std::result::Result<Self, tree_sitter::LanguageError> {
        let mut parser = Parser::new();
        let language = tree_sitter_gdshader::LANGUAGE;
        parser.set_language(&language.into())?;
        Ok(Self { parser })
    }

    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }
}

impl Default for GDShaderParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_shader() {
        let mut parser = GDShaderParser::new();
        let source = r#"
shader_type spatial;

uniform vec4 albedo_color : source_color = vec4(1.0);

void fragment() {
    ALBEDO = albedo_color.rgb;
}
"#;
        let tree = parser.parse(source).expect("Failed to parse");
        let root = tree.root_node();
        assert!(!root.has_error(), "Parse tree has errors");
    }

    #[test]
    fn test_parse_with_errors_recovers() {
        let mut parser = GDShaderParser::new();
        let source = "shader_type spatial;\nvoid fragment() {";
        let tree = parser.parse(source).expect("Failed to parse");
        let root = tree.root_node();
        // Tree-sitter still returns a tree, possibly with errors
        assert!(root.child_count() > 0);
    }
}
