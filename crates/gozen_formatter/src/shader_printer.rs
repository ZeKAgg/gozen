// GDShader AST-walking formatter following the Godot Shaders Style Guide:
// https://docs.godotengine.org/en/stable/tutorials/shaders/shaders_style_guide.html

use gozen_config::FormatterConfig;
use gozen_parser::{node_text, Node, Tree};

pub struct ShaderPrinter {
    output: String,
    indent_level: usize,
    config: FormatterConfig,
}

impl ShaderPrinter {
    pub fn new(config: &FormatterConfig) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            config: config.clone(),
        }
    }

    pub fn print(&mut self, tree: &Tree, source: &str) -> String {
        let root = tree.root_node();
        self.visit_node(root, source);
        // Ensure single trailing newline
        let trimmed = self.output.trim_end_matches('\n').to_string();
        self.output = trimmed;
        self.output.push('\n');
        self.output.clone()
    }

    // ── Core helpers ────────────────────────────────────────────────────

    fn indent_str(&self) -> String {
        if self.config.indent_style == "space" {
            " ".repeat(self.config.indent_width * self.indent_level)
        } else {
            "\t".repeat(self.indent_level)
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn emit_indent(&mut self) {
        let indent = self.indent_str();
        self.write(&indent);
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(1);
    }

    /// Emit a simple statement: indent, normalize spacing, newline.
    fn emit_simple_statement(&mut self, node: Node, source: &str) {
        let text = node_text(node, source).trim();
        if !text.is_empty() {
            self.emit_indent();
            self.write(&normalize_spacing(text));
            self.newline();
        }
    }

    // ── Central dispatch ────────────────────────────────────────────────

    fn visit_node(&mut self, node: Node, source: &str) {
        let kind = node.kind();

        // Error recovery
        if kind == "ERROR" || kind == "MISSING" {
            self.emit_simple_statement(node, source);
            return;
        }

        match kind {
            // Root node
            "source_file" | "translation_unit" => {
                self.visit_source_file(node, source);
            }

            // Top-level declarations
            "shader_type_declaration"
            | "render_mode_declaration"
            | "uniform_declaration"
            | "varying_declaration"
            | "const_declaration"
            | "group_uniforms_declaration" => {
                self.emit_simple_statement(node, source);
            }
            "include_declaration" => {
                let text = node_text(node, source).trim();
                if !text.is_empty() {
                    self.emit_indent();
                    self.write(text);
                    self.newline();
                }
            }
            "struct_declaration" => {
                self.visit_struct(node, source);
            }
            "function_declaration" => {
                self.visit_function(node, source);
            }

            // Control flow
            "if_statement" => self.visit_if(node, source),
            "for_statement" => self.visit_for(node, source),
            "while_statement" => self.visit_while(node, source),
            "switch_statement" => self.visit_switch(node, source),

            // Block container
            "block" => self.visit_block(node, source),
            "statement_sequence" => self.visit_statement_sequence(node, source),

            // Simple statements
            "var_declaration"
            | "const_var_declaration"
            | "assignment_statement"
            | "adjustment_statement"
            | "expr_statement"
            | "return_statement"
            | "break_statement"
            | "continue_statement"
            | "discard_statement" => {
                self.emit_simple_statement(node, source);
            }

            // Fallback: preserve as-is with normalized spacing
            _ => {
                self.emit_simple_statement(node, source);
            }
        }
    }

    // ── Source file ─────────────────────────────────────────────────────

    fn visit_source_file(&mut self, root: Node, source: &str) {
        let mut prev_kind: Option<String> = None;
        let mut i = 0;
        while i < root.child_count() {
            let child = match root.child(i) {
                Some(c) => c,
                None => {
                    i += 1;
                    continue;
                }
            };

            if !child.is_named() {
                i += 1;
                continue;
            }

            let kind = child.kind().to_string();

            // Error recovery
            if kind == "ERROR" || kind == "MISSING" {
                self.emit_simple_statement(child, source);
                prev_kind = Some(kind);
                i += 1;
                continue;
            }

            // Blank line between different declaration groups or before functions
            if let Some(ref prev) = prev_kind {
                let needs_blank = kind == "function_declaration"
                    || (prev != &kind
                        && prev != "shader_type_declaration"
                        && prev != "render_mode_declaration");
                if needs_blank {
                    self.newline();
                }
            }

            self.visit_node(child, source);
            // visit_node for top-level declarations already emits newlines,
            // but struct and function have their own logic. To avoid double-newline,
            // only add one if not already present.
            if !self.output.ends_with('\n') {
                self.newline();
            }

            prev_kind = Some(kind);
            i += 1;
        }
    }

    // ── Structs ─────────────────────────────────────────────────────────

    fn visit_struct(&mut self, node: Node, source: &str) {
        self.emit_indent();

        let mut parts = Vec::new();
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                parts.push(child);
            }
        }

        let mut wrote_open_brace = false;
        for child in &parts {
            let text = node_text(*child, source);
            let kind = child.kind();

            if kind == "struct_member_list" {
                self.newline();
                self.indent();
                self.visit_struct_members(*child, source);
                self.dedent();
                continue;
            }

            if text == "{" {
                self.write(" {");
                wrote_open_brace = true;
                continue;
            }
            if text == "}" {
                self.emit_indent();
                self.write("}");
                continue;
            }
            if text == ";" {
                self.write(";");
                continue;
            }
            if !wrote_open_brace && text == "struct" {
                self.write("struct ");
            } else if child.is_named() && !wrote_open_brace {
                self.write(text.trim());
            }
        }
        self.newline();
    }

    fn visit_struct_members(&mut self, node: Node, source: &str) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "struct_member" {
                    self.emit_indent();
                    let text = node_text(child, source).trim();
                    self.write(&normalize_spacing(text));
                    self.newline();
                }
            }
        }
    }

    // ── Functions ───────────────────────────────────────────────────────

    fn visit_function(&mut self, node: Node, source: &str) {
        self.emit_indent();

        let mut block_node = None;
        let mut return_type: Option<String> = None;
        let mut func_name: Option<String> = None;
        let mut params_text: Option<String> = None;

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let kind = child.kind();
                let text = node_text(child, source);

                if kind == "block" {
                    block_node = Some(child);
                    continue;
                }
                if !child.is_named() {
                    continue;
                }
                if kind == "parameter_list" {
                    params_text = Some(format_param_list(text));
                } else if func_name.is_none() && return_type.is_none() {
                    return_type = Some(text.trim().to_string());
                } else if func_name.is_none() {
                    func_name = Some(text.trim().to_string());
                }
            }
        }

        // Build header: "return_type name(params)"
        let mut header = String::new();
        if let Some(rt) = return_type {
            header.push_str(&rt);
        }
        if let Some(name) = func_name {
            if !header.is_empty() {
                header.push(' ');
            }
            header.push_str(&name);
        }
        if let Some(params) = params_text {
            let params = params.trim().to_string();
            if params.starts_with('(') {
                header.push_str(&params);
            } else {
                header.push('(');
                header.push_str(&params);
                header.push(')');
            }
        } else {
            header.push_str("()");
        }

        self.write(&header);

        if let Some(block) = block_node {
            self.write(" ");
            self.visit_block(block, source);
        }
        self.newline();
    }

    // ── Blocks ──────────────────────────────────────────────────────────

    fn visit_block(&mut self, node: Node, source: &str) {
        self.write("{");
        self.newline();
        self.indent();

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let text = node_text(child, source);
                if text == "{" || text == "}" {
                    continue;
                }
                if child.is_named() {
                    self.visit_node(child, source);
                }
            }
        }

        self.dedent();
        self.emit_indent();
        self.write("}");
    }

    fn visit_statement_sequence(&mut self, node: Node, source: &str) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.is_named() {
                    self.visit_node(child, source);
                }
            }
        }
    }

    // ── Control flow ────────────────────────────────────────────────────

    fn visit_if(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let mut i = 0;
        while i < node.child_count() {
            let child = match node.child(i) {
                Some(c) => c,
                None => {
                    i += 1;
                    continue;
                }
            };
            let kind = child.kind();
            let text = node_text(child, source);

            if text == "if" {
                self.write("if ");
            } else if text == "else" {
                self.write(" else ");
            } else if kind == "paren_expr" {
                self.write(&normalize_spacing(text.trim()));
            } else if kind == "block" {
                self.write(" ");
                self.visit_block(child, source);
            } else if kind == "_else_statement" {
                self.visit_else_branch(child, source);
            } else if child.is_named() {
                self.write(" ");
                self.visit_block_or_statement(child, source);
            }

            i += 1;
        }
        self.newline();
    }

    fn visit_else_branch(&mut self, node: Node, source: &str) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let text = node_text(child, source);
                let kind = child.kind();

                if text == "else" {
                    self.write(" else ");
                } else if kind == "block" {
                    self.visit_block(child, source);
                } else if kind == "if_statement" {
                    self.visit_inline_if(child, source);
                } else if child.is_named() {
                    self.visit_block_or_statement(child, source);
                }
            }
        }
    }

    fn visit_inline_if(&mut self, node: Node, source: &str) {
        let mut i = 0;
        while i < node.child_count() {
            let child = match node.child(i) {
                Some(c) => c,
                None => {
                    i += 1;
                    continue;
                }
            };
            let kind = child.kind();
            let text = node_text(child, source);

            if text == "if" {
                self.write("if ");
            } else if kind == "paren_expr" {
                self.write(&normalize_spacing(text.trim()));
            } else if kind == "block" {
                self.write(" ");
                self.visit_block(child, source);
            } else if kind == "_else_statement" {
                self.visit_else_branch(child, source);
            } else if child.is_named() {
                self.write(" ");
                self.visit_block_or_statement(child, source);
            }

            i += 1;
        }
    }

    fn visit_for(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let mut block_node = None;
        let mut header_end_byte = 0;

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "block" {
                    block_node = Some(child);
                    break;
                }
                header_end_byte = child.end_byte();
            }
        }

        let node_start = node.start_byte();
        let header_text = &source[node_start..header_end_byte];
        let header_text = normalize_spacing(header_text.trim());
        self.write(&header_text);

        if let Some(block) = block_node {
            self.write(" ");
            self.visit_block(block, source);
        }
        self.newline();
    }

    fn visit_while(&mut self, node: Node, source: &str) {
        self.emit_indent();
        self.write("while ");
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                let kind = child.kind();
                let text = node_text(child, source);

                if text == "while" {
                    continue;
                } else if kind == "paren_expr" {
                    self.write(&normalize_spacing(text.trim()));
                } else if kind == "block" {
                    self.write(" ");
                    self.visit_block(child, source);
                } else if child.is_named() {
                    self.write(" ");
                    self.visit_block_or_statement(child, source);
                }
            }
        }
        self.newline();
    }

    fn visit_switch(&mut self, node: Node, source: &str) {
        self.emit_indent();
        let text = node_text(node, source).trim();
        self.write(&normalize_spacing(text));
        self.newline();
    }

    fn visit_block_or_statement(&mut self, node: Node, source: &str) {
        if node.kind() == "block" {
            self.visit_block(node, source);
        } else {
            let text = node_text(node, source).trim();
            self.write(&normalize_spacing(text));
        }
    }
}

// ── Helper functions ────────────────────────────────────────────────────

/// Normalize spacing: collapse multiple spaces/tabs to single space.
fn normalize_spacing(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space && !result.is_empty() {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    result
}

/// Collapse multiple consecutive spaces into one.
fn collapse_spaces(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch == ' ' {
            if !prev_space {
                result.push(' ');
            }
            prev_space = true;
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    result
}

/// Format a parameter list, normalizing spaces after commas.
fn format_param_list(text: &str) -> String {
    let text = text.trim();
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == ',' {
            result.push(',');
            result.push(' ');
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }
        result.push(ch);
        i += 1;
    }
    collapse_spaces(&result)
}
