// Symbol index for Go To Definition / Find References / Document Symbols

use std::collections::HashMap;

use tower_lsp::lsp_types::{self, Position, Range};
use url::Url;

use gozen_parser::{first_identifier_child, node_text, walk_tree, GDScriptParser, Node, Tree};

/// What kind of symbol this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Class,
    Function,
    Signal,
    Constant,
    Enum,
    Variable,
    EnumMember,
}

impl SymbolKind {
    pub fn to_lsp(self) -> lsp_types::SymbolKind {
        match self {
            SymbolKind::Class => lsp_types::SymbolKind::CLASS,
            SymbolKind::Function => lsp_types::SymbolKind::FUNCTION,
            SymbolKind::Signal => lsp_types::SymbolKind::EVENT,
            SymbolKind::Constant => lsp_types::SymbolKind::CONSTANT,
            SymbolKind::Enum => lsp_types::SymbolKind::ENUM,
            SymbolKind::Variable => lsp_types::SymbolKind::VARIABLE,
            SymbolKind::EnumMember => lsp_types::SymbolKind::ENUM_MEMBER,
        }
    }
}

/// A single symbol definition or reference location.
#[derive(Debug, Clone)]
pub struct SymbolLocation {
    pub uri: Url,
    pub range: Range,
    pub kind: SymbolKind,
    pub name: String,
}

/// Aggregated symbol index for the project.
pub struct SymbolIndex {
    /// symbol name -> list of definitions
    pub definitions: HashMap<String, Vec<SymbolLocation>>,
    /// symbol name -> list of usage locations
    pub references: HashMap<String, Vec<SymbolLocation>>,
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolIndex {
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            references: HashMap::new(),
        }
    }

    /// Remove all entries for a given URI (used before re-indexing a file).
    pub fn remove_uri(&mut self, uri: &Url) {
        for locs in self.definitions.values_mut() {
            locs.retain(|l| l.uri != *uri);
        }
        self.definitions.retain(|_, v| !v.is_empty());

        for locs in self.references.values_mut() {
            locs.retain(|l| l.uri != *uri);
        }
        self.references.retain(|_, v| !v.is_empty());
    }

    /// Index a single file from its source and tree.
    pub fn index_file(&mut self, uri: &Url, source: &str, tree: &Tree) {
        self.remove_uri(uri);

        let root = tree.root_node();

        // Collect definitions
        walk_tree(root, source, |node, src| {
            let kind_str = node.kind();
            match kind_str {
                "class_name_statement" => {
                    if let Some(name_node) = first_identifier_child(node) {
                        let name = node_text(name_node, src).to_string();
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(name_node),
                            SymbolKind::Class,
                        );
                    }
                }
                "function_definition" => {
                    if let Some(name_node) = find_child_by_kind(node, "name") {
                        let name = node_text(name_node, src).to_string();
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(name_node),
                            SymbolKind::Function,
                        );
                    } else if let Some(name_node) = first_identifier_child(node) {
                        let name = node_text(name_node, src).to_string();
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(name_node),
                            SymbolKind::Function,
                        );
                    }
                }
                "signal_statement" | "signal_declaration" => {
                    if let Some(name_node) = first_identifier_child(node) {
                        let name = node_text(name_node, src).to_string();
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(name_node),
                            SymbolKind::Signal,
                        );
                    }
                }
                "variable_statement"
                | "export_variable_statement"
                | "onready_variable_statement" => {
                    let text = node_text(node, src);
                    let is_const = text.starts_with("const ");
                    if let Some(name_node) = first_identifier_child(node) {
                        let name = node_text(name_node, src).to_string();
                        let sym_kind = if is_const {
                            SymbolKind::Constant
                        } else {
                            SymbolKind::Variable
                        };
                        self.add_definition(&name, uri.clone(), node_range(name_node), sym_kind);
                    }
                }
                "enum_definition" => {
                    if let Some(name_node) = first_identifier_child(node) {
                        let name = node_text(name_node, src).to_string();
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(name_node),
                            SymbolKind::Enum,
                        );
                    }
                }
                _ => {}
            }
        });

        // Collect references (identifiers that are used, not defined)
        self.index_references(uri, source, root);
    }

    fn index_references(&mut self, uri: &Url, source: &str, root: Node) {
        walk_tree(root, source, |node, src| {
            if node.kind() != "identifier" {
                return;
            }

            // Skip if this identifier is a definition (child of function_definition name, etc.)
            if let Some(parent) = node.parent() {
                let pk = parent.kind();
                // If this identifier is the name being defined, skip
                if matches!(
                    pk,
                    "function_definition"
                        | "class_name_statement"
                        | "signal_statement"
                        | "signal_declaration"
                        | "variable_statement"
                        | "export_variable_statement"
                        | "onready_variable_statement"
                        | "enum_definition"
                ) {
                    // Check if this identifier is the first identifier child (the name being declared)
                    if let Some(first) = first_identifier_child(parent) {
                        if first.id() == node.id() {
                            return; // This is a definition, not a reference
                        }
                    }
                }
            }

            let name = node_text(node, src).to_string();
            if name.is_empty() || is_keyword(&name) {
                return;
            }

            self.references
                .entry(name.clone())
                .or_default()
                .push(SymbolLocation {
                    uri: uri.clone(),
                    range: node_range(node),
                    kind: SymbolKind::Variable, // We don't know the kind of a reference
                    name,
                });
        });
    }

    fn add_definition(&mut self, name: &str, uri: Url, range: Range, kind: SymbolKind) {
        self.definitions
            .entry(name.to_string())
            .or_default()
            .push(SymbolLocation {
                uri,
                range,
                kind,
                name: name.to_string(),
            });
    }

    /// Find definitions for a symbol name.
    pub fn find_definitions(&self, name: &str) -> Vec<&SymbolLocation> {
        self.definitions
            .get(name)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Find all references to a symbol name.
    pub fn find_references(&self, name: &str) -> Vec<&SymbolLocation> {
        self.references
            .get(name)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Get all document symbols for a given URI (for document outline).
    pub fn document_symbols(&self, uri: &Url) -> Vec<lsp_types::DocumentSymbol> {
        let mut symbols: Vec<lsp_types::DocumentSymbol> = Vec::new();

        for locs in self.definitions.values() {
            for loc in locs {
                if loc.uri == *uri {
                    #[allow(deprecated)]
                    symbols.push(lsp_types::DocumentSymbol {
                        name: loc.name.clone(),
                        detail: None,
                        kind: loc.kind.to_lsp(),
                        tags: None,
                        deprecated: None,
                        range: loc.range,
                        selection_range: loc.range,
                        children: None,
                    });
                }
            }
        }

        // Sort by start line
        symbols.sort_by_key(|s| (s.range.start.line, s.range.start.character));
        symbols
    }

    /// Index a GDShader file, extracting functions/uniforms/varyings/structs.
    pub fn index_shader_file(&mut self, uri: &Url, source: &str, tree: &Tree) {
        self.remove_uri(uri);
        let root = tree.root_node();

        walk_tree(root, source, |node, src| {
            match node.kind() {
                "function_declaration" => {
                    // Extract name: return_type name(...)
                    let text = node_text(node, src);
                    if let Some(name) = extract_shader_func_name(text) {
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(node),
                            SymbolKind::Function,
                        );
                    }
                }
                "uniform_declaration" => {
                    let text = node_text(node, src);
                    if let Some(name) = extract_shader_decl_name(text, "uniform") {
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(node),
                            SymbolKind::Variable,
                        );
                    }
                }
                "varying_declaration" => {
                    let text = node_text(node, src);
                    if let Some(name) = extract_shader_decl_name(text, "varying") {
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(node),
                            SymbolKind::Variable,
                        );
                    }
                }
                "const_declaration" => {
                    let text = node_text(node, src);
                    if let Some(name) = extract_shader_decl_name(text, "const") {
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(node),
                            SymbolKind::Constant,
                        );
                    }
                }
                "struct_declaration" => {
                    let text = node_text(node, src);
                    if let Some(name) = extract_shader_decl_name(text, "struct") {
                        self.add_definition(
                            &name,
                            uri.clone(),
                            node_range(node),
                            SymbolKind::Class,
                        );
                    }
                }
                _ => {}
            }
        });
    }

    /// Build the initial index from all .gd files using a parser.
    pub fn build_from_files(
        parser: &mut GDScriptParser,
        files: &HashMap<String, String>, // res_path -> content
        project_root: &std::path::Path,
    ) -> Self {
        let mut index = Self::new();

        for (res_path, content) in files {
            // Convert res:// path to a file URL
            let rel = res_path.strip_prefix("res://").unwrap_or(res_path);
            let abs = project_root.join(rel);
            let uri = match Url::from_file_path(&abs) {
                Ok(u) => u,
                Err(_) => continue,
            };

            if let Some(tree) = parser.parse(content) {
                index.index_file(&uri, content, &tree);
            }
        }

        index
    }
}

/// Convert a tree-sitter Node to an LSP Range.
fn node_range(node: Node) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start: Position {
            line: start.row as u32,
            character: start.column as u32,
        },
        end: Position {
            line: end.row as u32,
            character: end.column as u32,
        },
    }
}

/// Find a child node by its tree-sitter field name.
fn find_child_by_kind<'a>(node: Node<'a>, field_name: &str) -> Option<Node<'a>> {
    node.child_by_field_name(field_name)
}

/// Extract a function name from a shader function declaration text.
fn extract_shader_func_name(text: &str) -> Option<String> {
    // "return_type name(...)"
    let before_paren = text.split('(').next()?.trim();
    let parts: Vec<&str> = before_paren.split_whitespace().collect();
    parts.last().map(|s| s.to_string())
}

/// Extract a name from a shader declaration like "keyword type name..."
fn extract_shader_decl_name(text: &str, keyword: &str) -> Option<String> {
    let trimmed = text.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() || parts[0] != keyword {
        return None;
    }
    // For "struct Name {..." the name is parts[1]
    // For "uniform type name..." the name is parts[2]
    if keyword == "struct" {
        parts
            .get(1)
            .map(|s| s.trim_end_matches('{').trim().to_string())
    } else {
        // uniform/varying/const: keyword type name [: hint] [= value];
        parts
            .get(2)
            .map(|s| s.trim_end_matches([':', ';', '=']).to_string())
    }
}

/// Check if a name is a GDScript keyword (skip as references).
fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "elif"
            | "else"
            | "for"
            | "while"
            | "match"
            | "break"
            | "continue"
            | "pass"
            | "return"
            | "class"
            | "class_name"
            | "extends"
            | "is"
            | "as"
            | "self"
            | "signal"
            | "func"
            | "static"
            | "const"
            | "enum"
            | "var"
            | "onready"
            | "export"
            | "setget"
            | "tool"
            | "yield"
            | "assert"
            | "await"
            | "preload"
            | "in"
            | "not"
            | "and"
            | "or"
            | "true"
            | "false"
            | "null"
            | "void"
            | "PI"
            | "TAU"
            | "INF"
            | "NAN"
            | "super"
    )
}
