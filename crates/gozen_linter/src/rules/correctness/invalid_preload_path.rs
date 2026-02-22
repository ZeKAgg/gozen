use gozen_diagnostics::{Diagnostic, Severity};
use gozen_parser::{call_name, node_text, span_from_node, walk_tree, Tree};

use crate::rule::{Rule, RuleMetadata};

pub struct InvalidPreloadPath;

const METADATA: RuleMetadata = RuleMetadata {
    id: "correctness/invalidPreloadPath",
    name: "invalidPreloadPath",
    group: "correctness",
    default_severity: Severity::Error,
    has_fix: false,
    description: "preload() or load() referencing a file that does not exist.",
    explanation: "Ensure the path in preload() or load() exists relative to the project root.",
};

fn first_resolved_path_arg(node: gozen_parser::Node, source: &str) -> Option<String> {
    for i in 0..node.child_count() {
        let c = node.child(i)?;
        if c.kind() == "argument_list" || c.kind() == "arguments" {
            for j in 0..c.child_count() {
                if let Some(arg) = c.child(j) {
                    if arg.is_named() {
                        return resolve_constant_string_expr(node_text(arg, source));
                    }
                }
            }
            return None;
        }
    }
    None
}

fn resolve_constant_string_expr(expr: &str) -> Option<String> {
    let expr = strip_outer_parens(expr.trim());
    if expr.is_empty() {
        return None;
    }

    if let Some(s) = parse_string_literal(expr) {
        return Some(s);
    }

    let plus_parts = split_top_level(expr, '+');
    if plus_parts.len() > 1 {
        let mut out = String::new();
        for part in plus_parts {
            out.push_str(&resolve_constant_string_expr(part)?);
        }
        return Some(out);
    }

    if let Some((lhs, rhs)) = split_once_top_level(expr, '%') {
        let template = resolve_constant_string_expr(lhs)?;
        let values = if let Some(items) = parse_array_elements(rhs) {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve_scalar_literal(item)?);
            }
            out
        } else {
            vec![resolve_scalar_literal(rhs)?]
        };
        return apply_percent_format(&template, &values);
    }

    None
}

fn resolve_scalar_literal(expr: &str) -> Option<String> {
    let expr = strip_outer_parens(expr.trim());
    if let Some(s) = parse_string_literal(expr) {
        return Some(s);
    }
    if expr == "true" || expr == "false" || expr == "null" {
        return Some(expr.to_string());
    }
    if is_numeric_literal(expr) {
        return Some(expr.to_string());
    }
    None
}

fn parse_string_literal(expr: &str) -> Option<String> {
    let expr = expr.trim();
    if expr.len() < 2 {
        return None;
    }
    let first = expr.as_bytes()[0] as char;
    let last = expr.as_bytes()[expr.len() - 1] as char;
    if (first != '"' && first != '\'') || last != first {
        return None;
    }

    let mut out = String::new();
    let mut escape = false;
    for ch in expr[1..expr.len() - 1].chars() {
        if escape {
            match ch {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                '\'' => out.push('\''),
                other => out.push(other),
            }
            escape = false;
            continue;
        }
        if ch == first {
            // Unescaped inner quote means this is not a single literal token.
            return None;
        }
        if ch == '\\' {
            escape = true;
        } else {
            out.push(ch);
        }
    }
    if escape {
        out.push('\\');
    }
    Some(out)
}

fn apply_percent_format(template: &str, args: &[String]) -> Option<String> {
    let mut out = String::new();
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0usize;
    let mut arg_idx = 0usize;
    let mut used_placeholder = false;

    while i < chars.len() {
        if chars[i] != '%' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if i + 1 < chars.len() && chars[i + 1] == '%' {
            out.push('%');
            i += 2;
            continue;
        }

        used_placeholder = true;
        i += 1;
        let mut spec: Option<char> = None;
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_alphabetic() {
                spec = Some(c);
                i += 1;
                break;
            }
            i += 1;
        }
        let spec = spec?;
        let value = args.get(arg_idx)?;
        arg_idx += 1;
        match spec {
            's' | 'S' | 'd' | 'i' | 'u' | 'f' | 'F' | 'g' | 'G' | 'e' | 'E' | 'x' | 'X' | 'o' => {
                out.push_str(value)
            }
            _ => return None,
        }
    }

    if !used_placeholder || arg_idx != args.len() {
        return None;
    }
    Some(out)
}

fn parse_array_elements(expr: &str) -> Option<Vec<&str>> {
    let expr = strip_outer_parens(expr.trim());
    if !expr.starts_with('[') || !expr.ends_with(']') {
        return None;
    }
    let inner = &expr[1..expr.len() - 1];
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    Some(split_top_level(inner, ','))
}

fn strip_outer_parens(mut expr: &str) -> &str {
    loop {
        let trimmed = expr.trim();
        if trimmed.len() < 2 || !trimmed.starts_with('(') || !trimmed.ends_with(')') {
            return trimmed;
        }
        if !is_wrapped_by_outer_parens(trimmed) {
            return trimmed;
        }
        expr = &trimmed[1..trimmed.len() - 1];
    }
}

fn is_wrapped_by_outer_parens(expr: &str) -> bool {
    let mut round = 0i32;
    let mut square = 0i32;
    let mut curly = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for (idx, ch) in expr.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_single {
            if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }
        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '(' => round += 1,
            ')' => {
                round -= 1;
                if round == 0 && idx + ch.len_utf8() != expr.len() {
                    return false;
                }
            }
            '[' => square += 1,
            ']' => square -= 1,
            '{' => curly += 1,
            '}' => curly -= 1,
            _ => {}
        }
    }

    round == 0 && square == 0 && curly == 0 && !in_single && !in_double
}

fn split_once_top_level(expr: &str, op: char) -> Option<(&str, &str)> {
    let mut round = 0i32;
    let mut square = 0i32;
    let mut curly = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for (idx, ch) in expr.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_single {
            if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }
        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '(' => round += 1,
            ')' => round -= 1,
            '[' => square += 1,
            ']' => square -= 1,
            '{' => curly += 1,
            '}' => curly -= 1,
            _ => {
                if ch == op && round == 0 && square == 0 && curly == 0 {
                    let lhs = expr[..idx].trim();
                    let rhs = expr[idx + ch.len_utf8()..].trim();
                    if lhs.is_empty() || rhs.is_empty() {
                        return None;
                    }
                    return Some((lhs, rhs));
                }
            }
        }
    }
    None
}

fn split_top_level(expr: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut round = 0i32;
    let mut square = 0i32;
    let mut curly = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for (idx, ch) in expr.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_single {
            if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }
        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '(' => round += 1,
            ')' => round -= 1,
            '[' => square += 1,
            ']' => square -= 1,
            '{' => curly += 1,
            '}' => curly -= 1,
            _ => {
                if ch == sep && round == 0 && square == 0 && curly == 0 {
                    out.push(expr[start..idx].trim());
                    start = idx + ch.len_utf8();
                }
            }
        }
    }
    out.push(expr[start..].trim());
    out
}

fn is_numeric_literal(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars().peekable();
    if matches!(chars.peek(), Some('+') | Some('-')) {
        chars.next();
    }
    let mut has_digit = false;
    let mut has_dot = false;
    for ch in chars {
        if ch.is_ascii_digit() {
            has_digit = true;
            continue;
        }
        if ch == '.' && !has_dot {
            has_dot = true;
            continue;
        }
        return false;
    }
    has_digit
}

impl Rule for InvalidPreloadPath {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(
        &self,
        tree: &Tree,
        source: &str,
        context: Option<&crate::context::LintContext>,
    ) -> Vec<Diagnostic> {
        let project_root = match context.and_then(|c| c.project_root.as_ref()) {
            Some(r) => r,
            None => return Vec::new(),
        };
        let root = tree.root_node();
        let mut diags = Vec::new();
        walk_tree(root, source, |node, src| {
            if node.kind() != "call_expression" && node.kind() != "call" {
                return;
            }
            let name = call_name(node, src);
            if name != "preload" && name != "load" {
                return;
            }
            let Some(path_str) = first_resolved_path_arg(node, src) else {
                return;
            };
            if !path_str.starts_with("res://") {
                return;
            }
            let relative = path_str.strip_prefix("res://").unwrap_or(&path_str);
            // Normalize path separators and reject path traversal attempts
            let relative = relative.replace('\\', "/");
            if relative.contains("..") {
                return; // Reject path traversal
            }
            let resolved = project_root.join(&relative);
            // Ensure the resolved path is within the project root
            if !resolved.starts_with(project_root) {
                return;
            }
            if !resolved.exists() {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("Path does not exist: {}", path_str),
                    file_path: None,
                    rule_id: None,
                    span: span_from_node(node),
                    notes: vec![],
                    fix: None,
                });
            }
        });
        diags
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::context::LintContext;
    use crate::rule::Rule;
    use gozen_parser::GDScriptParser;

    use super::InvalidPreloadPath;

    fn temp_project_root() -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX_EPOCH")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gozen_invalid_preload_path_{ts}"));
        fs::create_dir_all(&root).expect("should create temp project root");
        root
    }

    fn lint_source(source: &str, project_root: PathBuf) -> Vec<gozen_diagnostics::Diagnostic> {
        let mut parser = GDScriptParser::new();
        let tree = parser
            .parse(source)
            .expect("fixture source should parse successfully");
        let ctx = LintContext {
            project_root: Some(project_root),
        };
        InvalidPreloadPath.check(&tree, source, Some(&ctx))
    }

    #[test]
    fn reports_missing_literal_path() {
        let root = temp_project_root();
        let source = "var scene = preload(\"res://scenes/missing.tscn\")";

        let diags = lint_source(source, root.clone());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("res://scenes/missing.tscn"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn skips_runtime_dynamic_percent_expression() {
        let root = temp_project_root();
        let source = "var scene = load(\"res://assets/%s/icon.png\" % [id])";

        let diags = lint_source(source, root.clone());
        assert!(diags.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_literal_percent_expression() {
        let root = temp_project_root();
        let source = "var scene = load(\"res://assets/%s/icon.png\" % [\"avatar\"] )";

        let diags = lint_source(source, root.clone());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("res://assets/avatar/icon.png"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_literal_concat_expression() {
        let root = temp_project_root();
        let source = "var scene = load(\"res://assets/\" + \"avatar\" + \"/icon.png\")";

        let diags = lint_source(source, root.clone());
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("res://assets/avatar/icon.png"));

        let _ = fs::remove_dir_all(root);
    }
}
