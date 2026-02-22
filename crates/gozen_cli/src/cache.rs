//! Content-hash caches for formatter and lint diagnostics.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use gozen_config::GozenConfig;
use gozen_diagnostics::{Diagnostic, Note, Severity, Span};

/// Legacy type name kept for compatibility in callers.
pub type GozenCache = FormatCache;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormatCacheEntry {
    pub content_hash: u64,
    pub config_hash: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FormatCache {
    pub version: u32,
    #[serde(default)]
    pub gozen_version: String,
    pub entries: HashMap<String, FormatCacheEntry>,
}

const FORMAT_CACHE_VERSION: u32 = 1;
const CACHE_DIR: &str = ".godot/gozen_cache";
const FORMAT_CACHE_FILE: &str = "format_v1.json";

impl FormatCache {
    pub fn load(project_root: &Path) -> Self {
        let new_path = format_cache_path(project_root);
        if let Ok(content) = std::fs::read_to_string(&new_path) {
            if let Ok(cache) = serde_json::from_str::<FormatCache>(&content) {
                if cache.version == FORMAT_CACHE_VERSION
                    && (cache.gozen_version.is_empty()
                        || cache.gozen_version == env!("CARGO_PKG_VERSION"))
                {
                    return cache;
                }
            }
        }

        Self::default()
    }

    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = format_cache_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self).unwrap_or_default();
        std::fs::write(path, json)
    }

    pub fn is_cached(&self, file_path: &str, content_hash: u64, config_hash: u64) -> bool {
        self.entries.get(file_path).is_some_and(|entry| {
            entry.content_hash == content_hash && entry.config_hash == config_hash
        })
    }

    pub fn update(&mut self, file_path: String, content_hash: u64, config_hash: u64) {
        self.version = FORMAT_CACHE_VERSION;
        self.gozen_version = env!("CARGO_PKG_VERSION").to_string();
        self.entries.insert(
            file_path,
            FormatCacheEntry {
                content_hash,
                config_hash,
            },
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CachedSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSpan {
    start_byte: usize,
    end_byte: usize,
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedNote {
    message: String,
    span: Option<CachedSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDiagnostic {
    severity: CachedSeverity,
    message: String,
    file_path: Option<String>,
    rule_id: Option<String>,
    span: CachedSpan,
    notes: Vec<CachedNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintCacheEntry {
    pub content_hash: u64,
    pub lint_config_hash: u64,
    pub project_fingerprint_hash: u64,
    diagnostics: Vec<CachedDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintCache {
    pub version: u32,
    #[serde(default)]
    pub gozen_version: String,
    pub entries: HashMap<String, LintCacheEntry>,
}

const LINT_CACHE_VERSION: u32 = 1;
const LINT_CACHE_FILE: &str = "lint_v1.json";

impl LintCache {
    pub fn load(project_root: &Path) -> Self {
        let path = lint_cache_path(project_root);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        match serde_json::from_str::<LintCache>(&content) {
            Ok(cache)
                if cache.version == LINT_CACHE_VERSION
                    && (cache.gozen_version.is_empty()
                        || cache.gozen_version == env!("CARGO_PKG_VERSION")) =>
            {
                cache
            }
            _ => Self::default(),
        }
    }

    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = lint_cache_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self).unwrap_or_default();
        std::fs::write(path, json)
    }

    pub fn lookup(
        &self,
        file_path: &str,
        content_hash: u64,
        lint_config_hash: u64,
        project_fingerprint_hash: u64,
    ) -> Option<Vec<Diagnostic>> {
        let entry = self.entries.get(file_path)?;
        if entry.content_hash != content_hash
            || entry.lint_config_hash != lint_config_hash
            || entry.project_fingerprint_hash != project_fingerprint_hash
        {
            return None;
        }
        Some(entry.diagnostics.iter().map(cached_to_diagnostic).collect())
    }

    pub fn update(
        &mut self,
        file_path: String,
        content_hash: u64,
        lint_config_hash: u64,
        project_fingerprint_hash: u64,
        diagnostics: &[Diagnostic],
    ) {
        self.version = LINT_CACHE_VERSION;
        self.gozen_version = env!("CARGO_PKG_VERSION").to_string();
        self.entries.insert(
            file_path,
            LintCacheEntry {
                content_hash,
                lint_config_hash,
                project_fingerprint_hash,
                diagnostics: diagnostics.iter().map(diagnostic_to_cached).collect(),
            },
        );
    }
}

fn diagnostic_to_cached(d: &Diagnostic) -> CachedDiagnostic {
    CachedDiagnostic {
        severity: match d.severity {
            Severity::Error => CachedSeverity::Error,
            Severity::Warning => CachedSeverity::Warning,
            Severity::Info => CachedSeverity::Info,
        },
        message: d.message.clone(),
        file_path: d.file_path.clone(),
        rule_id: d.rule_id.clone(),
        span: span_to_cached(d.span),
        notes: d
            .notes
            .iter()
            .map(|n| CachedNote {
                message: n.message.clone(),
                span: n.span.map(span_to_cached),
            })
            .collect(),
    }
}

fn cached_to_diagnostic(d: &CachedDiagnostic) -> Diagnostic {
    Diagnostic {
        severity: match d.severity {
            CachedSeverity::Error => Severity::Error,
            CachedSeverity::Warning => Severity::Warning,
            CachedSeverity::Info => Severity::Info,
        },
        message: d.message.clone(),
        file_path: d.file_path.clone(),
        rule_id: d.rule_id.clone(),
        span: cached_to_span(&d.span),
        notes: d
            .notes
            .iter()
            .map(|n| Note {
                message: n.message.clone(),
                span: n.span.as_ref().map(cached_to_span),
            })
            .collect(),
        fix: None,
    }
}

fn span_to_cached(s: Span) -> CachedSpan {
    CachedSpan {
        start_byte: s.start_byte,
        end_byte: s.end_byte,
        start_row: s.start_row,
        start_col: s.start_col,
        end_row: s.end_row,
        end_col: s.end_col,
    }
}

fn cached_to_span(s: &CachedSpan) -> Span {
    Span {
        start_byte: s.start_byte,
        end_byte: s.end_byte,
        start_row: s.start_row,
        start_col: s.start_col,
        end_row: s.end_row,
        end_col: s.end_col,
    }
}

pub fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

pub fn hash_config(config: &gozen_config::FormatterConfig) -> u64 {
    hash_format_config(config)
}

pub fn hash_format_config(config: &gozen_config::FormatterConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    config.indent_style.hash(&mut hasher);
    config.indent_width.hash(&mut hasher);
    config.line_width.hash(&mut hasher);
    config.trailing_comma.hash(&mut hasher);
    config.end_of_line.hash(&mut hasher);
    hasher.finish()
}

pub fn hash_lint_config(config: &GozenConfig) -> u64 {
    hash_content(&format!(
        "{:?}|{:?}|{:?}",
        config.linter, config.analyzer, config.shader
    ))
}

pub fn compute_project_fingerprint(project_root: &Path) -> u64 {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".godot" && !name.starts_with(".git")
        })
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let include = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "project.godot")
            || path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| ext == "gd" || ext == "tscn" || ext == "tres");
        if include {
            files.push(path.to_path_buf());
        }
    }

    files.sort();

    let mut hasher = DefaultHasher::new();
    for path in files {
        if let Ok(rel) = path.strip_prefix(project_root) {
            rel.to_string_lossy().hash(&mut hasher);
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            content.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn format_cache_path(project_root: &Path) -> PathBuf {
    project_root.join(CACHE_DIR).join(FORMAT_CACHE_FILE)
}

fn lint_cache_path(project_root: &Path) -> PathBuf {
    project_root.join(CACHE_DIR).join(LINT_CACHE_FILE)
}

pub fn find_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        if dir.join("project.godot").exists() || dir.join(".godot").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
