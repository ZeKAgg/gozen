// LSP server — diagnostics, formatting over stdin/stdout

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use url::Url;

use gozen_config::GozenConfig;
use gozen_formatter::{format as fmt, format_shader};
use gozen_linter::{LintContext, LintEngine};
use gozen_parser::{GDScriptParser, GDShaderParser};
use gozen_project::ProjectGraph;

use crate::project_queries::{
    collect_project_groups, completion_context_at_position, extract_node_path_at_position,
    gather_signal_usage_locations, node_path_completions_for_prefix,
    resolve_node_definition_locations, uri_to_res_path, CompletionContextKind,
};
use crate::symbol_index::SymbolIndex;
use crate::watcher;

const MAX_LSP_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024;

fn uri_to_path(uri: &Url) -> PathBuf {
    uri.to_file_path().unwrap_or_else(|_| {
        let path = uri.path();
        // On Windows, uri.path() returns "/C:/..." which is invalid.
        // Strip the leading slash when a drive letter is present.
        #[cfg(target_os = "windows")]
        {
            let stripped = path.strip_prefix('/').unwrap_or(path);
            if stripped.len() >= 2 && stripped.as_bytes()[1] == b':' {
                return PathBuf::from(stripped);
            }
        }
        PathBuf::from(path)
    })
}

/// Convert an absolute path to a res:// path relative to the project root.
fn to_res_path(abs_path: &Path, project_root: &Path) -> String {
    let rel = abs_path.strip_prefix(project_root).unwrap_or(abs_path);
    format!("res://{}", rel.to_string_lossy().replace('\\', "/"))
}

fn read_text_file_limited(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_LSP_FILE_SIZE_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Convert an LSP Position (line, character) to a byte offset within `doc`.
/// Handles both LF and CRLF line endings correctly.
fn position_to_offset(doc: &str, position: Position) -> Option<usize> {
    let mut offset = 0usize;
    let mut current_line = 0u32;
    let target_line = position.line;
    let target_char = position.character as usize;

    for line in doc.split('\n') {
        // Strip trailing \r so character offsets are correct for CRLF documents
        let line_content = line.strip_suffix('\r').unwrap_or(line);
        if current_line == target_line {
            // Clamp the character offset to the visible line length (without \r)
            let char_offset = target_char.min(line_content.len());
            return Some(offset + char_offset);
        }
        // +1 for the '\n' that split() removed; line.len() includes any \r
        offset += line.len() + 1;
        current_line += 1;
    }

    // If we're past all lines, return the end of the document
    if current_line == target_line {
        return Some(offset.min(doc.len()));
    }
    None
}

fn diagnostic_to_lsp(d: &gozen_diagnostics::Diagnostic) -> Diagnostic {
    let severity = match d.severity {
        gozen_diagnostics::Severity::Error => Some(DiagnosticSeverity::ERROR),
        gozen_diagnostics::Severity::Warning => Some(DiagnosticSeverity::WARNING),
        gozen_diagnostics::Severity::Info => Some(DiagnosticSeverity::INFORMATION),
    };
    Diagnostic {
        range: Range {
            start: Position {
                line: d.span.start_row as u32,
                character: d.span.start_col as u32,
            },
            end: Position {
                line: d.span.end_row as u32,
                character: d.span.end_col as u32,
            },
        },
        severity,
        code: d.rule_id.clone().map(NumberOrString::String),
        source: Some("gozen".into()),
        message: d.message.clone(),
        related_information: None,
        tags: None,
        code_description: None,
        data: None,
    }
}

fn full_range(source: &str) -> Range {
    let lines: usize = source.lines().count();
    let last_line_len = source.lines().last().map(|l| l.len()).unwrap_or(0);
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: lines.saturating_sub(1) as u32,
            character: last_line_len as u32,
        },
    }
}

pub struct GozenLspBackend {
    client: Client,
    documents: Arc<tokio::sync::RwLock<HashMap<Url, String>>>,
    /// Stores the full gozen diagnostics (including Fix objects) per document.
    gozen_diagnostics: Arc<tokio::sync::RwLock<HashMap<Url, Vec<gozen_diagnostics::Diagnostic>>>>,
    parser: Arc<std::sync::Mutex<GDScriptParser>>,
    shader_parser: Arc<std::sync::Mutex<GDShaderParser>>,
    config: GozenConfig,
    lint_engine: LintEngine,
    project_root: Option<PathBuf>,
    project_graph: Arc<tokio::sync::RwLock<Option<ProjectGraph>>>,
    symbol_index: Arc<tokio::sync::RwLock<SymbolIndex>>,
}

impl GozenLspBackend {
    pub fn new(client: Client, config: GozenConfig) -> Self {
        let project_root = std::env::current_dir().ok();
        let project_graph = project_root
            .as_ref()
            .filter(|_| config.analyzer.project_graph)
            .and_then(|root| ProjectGraph::build(root).ok());
        let lint_engine = LintEngine::new_full(
            &config.linter,
            config.analyzer.project_graph,
            &config.shader,
        );

        // Build initial symbol index from all scripts in the project
        let symbol_index =
            if let (Some(ref root), Some(ref graph)) = (&project_root, &project_graph) {
                let files: HashMap<String, String> = graph
                    .scripts
                    .keys()
                    .filter_map(|res_path| {
                        let rel = res_path.strip_prefix("res://").unwrap_or(res_path);
                        let abs = root.join(rel);
                        read_text_file_limited(&abs).map(|c| (res_path.clone(), c))
                    })
                    .collect();
                let mut parser = GDScriptParser::new();
                SymbolIndex::build_from_files(&mut parser, &files, root)
            } else {
                SymbolIndex::new()
            };

        Self {
            client,
            documents: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            gozen_diagnostics: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            parser: Arc::new(std::sync::Mutex::new(GDScriptParser::new())),
            shader_parser: Arc::new(std::sync::Mutex::new(GDShaderParser::new())),
            config,
            lint_engine,
            project_root,
            project_graph: Arc::new(tokio::sync::RwLock::new(project_graph)),
            symbol_index: Arc::new(tokio::sync::RwLock::new(symbol_index)),
        }
    }

    async fn get_document(&self, uri: &Url) -> Option<String> {
        self.documents.read().await.get(uri).cloned()
    }

    /// Re-index a file's symbols after it changes.
    /// Parses first (holding only the parser mutex), then acquires the index
    /// write lock to avoid holding both simultaneously.
    async fn update_symbol_index(&self, uri: &Url, source: &str) {
        let path_str = uri.path();
        let is_shader = path_str.ends_with(".gdshader");
        // Parse outside the index write-lock scope to reduce contention
        let tree = if is_shader {
            let mut parser = self.shader_parser.lock().unwrap_or_else(|e| e.into_inner());
            parser.parse(source)
        } else {
            let mut parser = self.parser.lock().unwrap_or_else(|e| e.into_inner());
            parser.parse(source)
        };
        if let Some(tree) = tree {
            let mut index = self.symbol_index.write().await;
            if is_shader {
                index.index_shader_file(uri, source, &tree);
            } else {
                index.index_file(uri, source, &tree);
            }
        }
    }

    /// Find the word (identifier) at the given position in source text.
    fn word_at_position(source: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = source.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return None;
        }
        let line = lines[line_idx];
        let col = position.character as usize;
        if col > line.len() {
            return None;
        }
        let bytes = line.as_bytes();

        // Find start of word
        let mut start = col;
        while start > 0 && is_ident_char(bytes[start - 1]) {
            start -= 1;
        }

        // Find end of word
        let mut end = col;
        while end < bytes.len() && is_ident_char(bytes[end]) {
            end += 1;
        }

        if start == end {
            return None;
        }
        Some(line[start..end].to_string())
    }

    fn is_valid_identifier(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return false;
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    async fn publish_diagnostics_for(&self, uri: Url, source: &str) {
        let path = uri_to_path(&uri);
        let path_str = path.to_string_lossy();
        let is_shader = path_str.ends_with(".gdshader");
        let is_gd = path_str.ends_with(".gd");

        if !is_gd && !is_shader {
            self.client
                .publish_diagnostics(uri.clone(), vec![], None)
                .await;
            self.gozen_diagnostics.write().await.remove(&uri);
            return;
        }

        if is_shader {
            let tree = {
                let mut parser = self.shader_parser.lock().unwrap_or_else(|e| e.into_inner());
                parser.parse(source)
            };
            let tree = match tree {
                Some(t) => t,
                None => {
                    self.client
                        .publish_diagnostics(uri.clone(), vec![], None)
                        .await;
                    self.gozen_diagnostics.write().await.remove(&uri);
                    return;
                }
            };
            let diags = self.lint_engine.lint_shader(&tree, source, &path_str);
            let lsp_diags: Vec<Diagnostic> = diags.iter().map(diagnostic_to_lsp).collect();
            self.gozen_diagnostics
                .write()
                .await
                .insert(uri.clone(), diags);
            self.client.publish_diagnostics(uri, lsp_diags, None).await;
        } else {
            let tree = {
                let mut parser = self.parser.lock().unwrap_or_else(|e| e.into_inner());
                parser.parse(source)
            };
            let tree = match tree {
                Some(t) => t,
                None => {
                    self.client
                        .publish_diagnostics(uri.clone(), vec![], None)
                        .await;
                    self.gozen_diagnostics.write().await.remove(&uri);
                    return;
                }
            };
            let context = self.project_root.as_ref().map(|r| LintContext {
                project_root: Some(r.clone()),
            });
            let script_res_path = self
                .project_root
                .as_ref()
                .and_then(|root| path.strip_prefix(root).ok())
                .map(|rel| format!("res://{}", rel.to_string_lossy().replace('\\', "/")));
            let graph_guard = self.project_graph.read().await;
            let graph = graph_guard.as_ref();
            let diags = self.lint_engine.lint(
                &tree,
                source,
                &path_str,
                context.as_ref(),
                graph,
                script_res_path.as_deref(),
            );
            let lsp_diags: Vec<Diagnostic> = diags.iter().map(diagnostic_to_lsp).collect();
            self.gozen_diagnostics
                .write()
                .await
                .insert(uri.clone(), diags);
            self.client.publish_diagnostics(uri, lsp_diags, None).await;
        }
    }
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Convert a gozen_diagnostics::Span to an LSP Range.
fn span_to_range(span: &gozen_diagnostics::Span) -> Range {
    Range {
        start: Position {
            line: span.start_row as u32,
            character: span.start_col as u32,
        },
        end: Position {
            line: span.end_row as u32,
            character: span.end_col as u32,
        },
    }
}

/// Check whether two LSP ranges overlap (share any line range).
fn ranges_overlap(a: &Range, b: &Range) -> bool {
    !(a.end.line < b.start.line || b.end.line < a.start.line)
}

#[tower_lsp::async_trait]
impl LanguageServer for GozenLspBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "\"".to_string(),
                        "$".to_string(),
                        ".".to_string(),
                        "/".to_string(),
                    ]),
                    all_commit_characters: None,
                    completion_item: None,
                    work_done_progress_options: Default::default(),
                }),
                rename_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "gozen".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Gozen LSP initialized")
            .await;

        // Start file watcher if we have a project root
        if let Some(ref root) = self.project_root {
            let root = root.clone();
            let graph = Arc::clone(&self.project_graph);
            let index = Arc::clone(&self.symbol_index);
            let parser = Arc::clone(&self.parser);
            let client = self.client.clone();
            let open_documents = Arc::clone(&self.documents);

            tokio::spawn(async move {
                run_file_watcher(root, graph, index, parser, client, open_documents).await;
            });
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        // Clone text for the document store; original is borrowed for diagnostics
        self.documents
            .write()
            .await
            .insert(uri.clone(), text.clone());
        self.update_symbol_index(&uri, &text).await;
        // Last use of uri -- moved, not cloned
        self.publish_diagnostics_for(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let mut documents = self.documents.write().await;
        let doc = match documents.get_mut(&uri) {
            Some(d) => d,
            None => return,
        };
        for change in params.content_changes {
            match change.range {
                Some(range) => {
                    let start = match position_to_offset(doc, range.start) {
                        Some(o) => o,
                        None => continue,
                    };
                    let end = match position_to_offset(doc, range.end) {
                        Some(o) => o,
                        None => continue,
                    };
                    if start <= doc.len() && end <= doc.len() && start <= end {
                        doc.replace_range(start..end, &change.text);
                    }
                }
                None => *doc = change.text,
            }
        }
        let text = doc.clone();
        drop(documents);
        self.update_symbol_index(&uri, &text).await;
        self.publish_diagnostics_for(uri, &text).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let source = match params.text {
            Some(text) => {
                // Clone text for the document store; original is borrowed for diagnostics
                self.documents
                    .write()
                    .await
                    .insert(uri.clone(), text.clone());
                text
            }
            None => self.get_document(&uri).await.unwrap_or_default(),
        };
        self.publish_diagnostics_for(uri, &source).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);
        self.gozen_diagnostics.write().await.remove(&uri);
        // If the file no longer exists on disk, remove stale symbols from the index.
        // Otherwise keep project-wide symbols available for cross-file navigation.
        let path = uri_to_path(&uri);
        if !path.exists() {
            self.symbol_index.write().await.remove_uri(&uri);
        }
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let requested_range = params.range;

        let diags_guard = self.gozen_diagnostics.read().await;
        let gozen_diags = match diags_guard.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let mut actions: Vec<CodeActionOrCommand> = Vec::new();

        for diag in gozen_diags {
            let fix = match &diag.fix {
                Some(f) if !f.changes.is_empty() => f,
                _ => continue,
            };

            let diag_range = span_to_range(&diag.span);
            if !ranges_overlap(&diag_range, &requested_range) {
                continue;
            }

            // Convert gozen TextEdits to LSP TextEdits
            let edits: Vec<TextEdit> = fix
                .changes
                .iter()
                .map(|change| TextEdit {
                    range: span_to_range(&change.span),
                    new_text: change.new_text.clone(),
                })
                .collect();

            let mut workspace_edit_changes = HashMap::new();
            workspace_edit_changes.insert(uri.clone(), edits);

            let lsp_diag = diagnostic_to_lsp(diag);

            let action = CodeAction {
                title: fix.description.clone(),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![lsp_diag]),
                edit: Some(WorkspaceEdit {
                    changes: Some(workspace_edit_changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                is_preferred: Some(fix.is_safe),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(action));
        }

        if actions.is_empty() {
            return Ok(None);
        }
        Ok(Some(actions))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let source = match self.get_document(&uri).await {
            Some(s) => s,
            None => return Ok(None),
        };

        // Project-aware node path navigation: $Foo/Bar or get_node("Foo/Bar")
        if let Some(root) = &self.project_root {
            let graph_guard = self.project_graph.read().await;
            if let Some(graph) = graph_guard.as_ref() {
                if let (Some(script_res_path), Some(node_path)) = (
                    uri_to_res_path(&uri, root),
                    extract_node_path_at_position(&source, position),
                ) {
                    let locations = resolve_node_definition_locations(
                        graph,
                        root,
                        &script_res_path,
                        &node_path,
                    );
                    if !locations.is_empty() {
                        if locations.len() == 1 {
                            return Ok(Some(GotoDefinitionResponse::Scalar(locations[0].clone())));
                        }
                        return Ok(Some(GotoDefinitionResponse::Array(locations)));
                    }
                }
            }
        }

        let word = match Self::word_at_position(&source, position) {
            Some(w) => w,
            None => return Ok(None),
        };

        let index = self.symbol_index.read().await;
        let defs = index.find_definitions(&word);

        if defs.is_empty() {
            // Try class_name from project graph
            let graph_guard = self.project_graph.read().await;
            if let Some(graph) = graph_guard.as_ref() {
                if let Some(script_path) = graph.class_names.get(&word) {
                    if let Some(root) = &self.project_root {
                        let rel = script_path.strip_prefix("res://").unwrap_or(script_path);
                        let abs = root.join(rel);
                        if let Ok(target_uri) = Url::from_file_path(&abs) {
                            // Try to find the actual class_name definition line in the target file
                            let target_range =
                                read_text_file_limited(&abs).and_then(|content| {
                                    content.lines().enumerate().find_map(|(i, line)| {
                                        let trimmed = line.trim();
                                        if trimmed.starts_with("class_name")
                                            && trimmed.contains(&word)
                                        {
                                            let col = line.find("class_name").unwrap_or(0) as u32;
                                            Some(Range {
                                                start: Position {
                                                    line: i as u32,
                                                    character: col,
                                                },
                                                end: Position {
                                                    line: i as u32,
                                                    character: col + trimmed.len() as u32,
                                                },
                                            })
                                        } else {
                                            None
                                        }
                                    })
                                })
                                .unwrap_or_default();
                            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                uri: target_uri,
                                range: target_range,
                            })));
                        }
                    }
                }
            }
            return Ok(None);
        }

        if defs.len() == 1 {
            Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: defs[0].uri.clone(),
                range: defs[0].range,
            })))
        } else {
            let locations: Vec<Location> = defs
                .iter()
                .map(|d| Location {
                    uri: d.uri.clone(),
                    range: d.range,
                })
                .collect();
            Ok(Some(GotoDefinitionResponse::Array(locations)))
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let source = match self.get_document(&uri).await {
            Some(s) => s,
            None => return Ok(None),
        };

        let word = match Self::word_at_position(&source, position) {
            Some(w) => w,
            None => return Ok(None),
        };

        let index = self.symbol_index.read().await;
        let mut locations: Vec<Location> = Vec::new();
        let is_signal_symbol = index
            .find_definitions(&word)
            .iter()
            .any(|d| d.kind == crate::symbol_index::SymbolKind::Signal);

        // Include definitions if requested
        if params.context.include_declaration {
            for def in index.find_definitions(&word) {
                locations.push(Location {
                    uri: def.uri.clone(),
                    range: def.range,
                });
            }
        }

        // Include all references
        for reference in index.find_references(&word) {
            locations.push(Location {
                uri: reference.uri.clone(),
                range: reference.range,
            });
        }
        drop(index);

        if is_signal_symbol {
            let open_docs = self.documents.read().await.clone();
            if let Some(root) = &self.project_root {
                let graph_guard = self.project_graph.read().await;
                if let Some(graph) = graph_guard.as_ref() {
                    let mut signal_locs =
                        gather_signal_usage_locations(graph, root, &open_docs, &word);
                    locations.append(&mut signal_locs);
                }
            }
        }

        let mut seen = std::collections::HashSet::new();
        locations.retain(|loc| {
            let key = format!(
                "{}:{}:{}:{}:{}",
                loc.uri,
                loc.range.start.line,
                loc.range.start.character,
                loc.range.end.line,
                loc.range.end.character
            );
            seen.insert(key)
        });
        locations.sort_by(|a, b| {
            (a.uri.as_str(), a.range.start.line, a.range.start.character).cmp(&(
                b.uri.as_str(),
                b.range.start.line,
                b.range.start.character,
            ))
        });

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        let index = self.symbol_index.read().await;
        let symbols = index.document_symbols(&uri);

        if symbols.is_empty() {
            return Ok(None);
        }

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let source = match self.get_document(&uri).await {
            Some(s) => s,
            None => return Ok(None),
        };
        let Some(ctx) = completion_context_at_position(&source, position) else {
            return Ok(None);
        };
        let Some(root) = &self.project_root else {
            return Ok(None);
        };
        let open_docs = self.documents.read().await.clone();
        let graph_guard = self.project_graph.read().await;
        let Some(graph) = graph_guard.as_ref() else {
            return Ok(None);
        };

        let labels: Vec<String> = match ctx {
            CompletionContextKind::NodePath { prefix } => {
                let Some(script_res_path) = uri_to_res_path(&uri, root) else {
                    return Ok(None);
                };
                node_path_completions_for_prefix(graph, &script_res_path, &prefix)
            }
            CompletionContextKind::Group { prefix } => {
                collect_project_groups(graph, root, &open_docs)
                    .into_iter()
                    .filter(|g| g.starts_with(&prefix))
                    .collect()
            }
            CompletionContextKind::InputAction { prefix } => graph
                .input_actions
                .iter()
                .filter(|a| a.starts_with(&prefix))
                .cloned()
                .collect(),
        };

        if labels.is_empty() {
            return Ok(None);
        }

        let items: Vec<CompletionItem> = labels
            .into_iter()
            .map(|label| CompletionItem {
                label: label.clone(),
                kind: Some(CompletionItemKind::VALUE),
                insert_text: Some(label),
                ..Default::default()
            })
            .collect();
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        if !Self::is_valid_identifier(&params.new_name) {
            return Ok(None);
        }
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let source = match self.get_document(&uri).await {
            Some(s) => s,
            None => return Ok(None),
        };
        let word = match Self::word_at_position(&source, position) {
            Some(w) => w,
            None => return Ok(None),
        };

        let index = self.symbol_index.read().await;
        let defs = index.find_definitions(&word);
        if defs.is_empty() {
            return Ok(None);
        }
        let refs = index.find_references(&word);

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        let mut seen = std::collections::HashSet::new();
        for loc in defs.into_iter().chain(refs.into_iter()) {
            let key = format!(
                "{}:{}:{}:{}:{}",
                loc.uri,
                loc.range.start.line,
                loc.range.start.character,
                loc.range.end.line,
                loc.range.end.character
            );
            if !seen.insert(key) {
                continue;
            }
            changes.entry(loc.uri.clone()).or_default().push(TextEdit {
                range: loc.range,
                new_text: params.new_name.clone(),
            });
        }
        if changes.is_empty() {
            return Ok(None);
        }
        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let source = match self.get_document(&uri).await {
            Some(s) => s,
            None => return Ok(None),
        };
        let is_shader = uri.path().ends_with(".gdshader");
        let formatted = if is_shader {
            let tree = {
                let mut parser = self.shader_parser.lock().unwrap_or_else(|e| e.into_inner());
                parser.parse(&source)
            };
            let tree = match tree {
                Some(t) => t,
                None => return Ok(None),
            };
            format_shader(&source, &tree, &self.config.formatter)
        } else {
            let tree = {
                let mut parser = self.parser.lock().unwrap_or_else(|e| e.into_inner());
                parser.parse(&source)
            };
            let tree = match tree {
                Some(t) => t,
                None => return Ok(None),
            };
            fmt(&source, &tree, &self.config.formatter)
        };
        if formatted == source {
            return Ok(None);
        }
        Ok(Some(vec![TextEdit {
            range: full_range(&source),
            new_text: formatted,
        }]))
    }
}

/// Run the file watcher loop, dispatching events to update the project graph
/// and symbol index. Extracted from `initialized()` for readability.
async fn run_file_watcher(
    root: PathBuf,
    graph: Arc<tokio::sync::RwLock<Option<ProjectGraph>>>,
    index: Arc<tokio::sync::RwLock<crate::symbol_index::SymbolIndex>>,
    parser: Arc<std::sync::Mutex<GDScriptParser>>,
    client: Client,
    open_documents: Arc<tokio::sync::RwLock<HashMap<Url, String>>>,
) {
    let (watcher, mut rx) = match watcher::start_watching(root.clone()) {
        Ok(w) => w,
        Err(e) => {
            client
                .log_message(
                    MessageType::WARNING,
                    format!("Failed to start file watcher: {}", e),
                )
                .await;
            return;
        }
    };

    // Keep watcher alive for the duration of the loop
    let _watcher = watcher;

    // Debounce: collect events for 100ms then process
    loop {
        let evt = match rx.recv().await {
            Some(e) => e,
            None => break,
        };

        // Collect any additional events within debounce window
        let mut events = vec![evt];
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }

        for event in events {
            handle_file_event(event, &root, &graph, &index, &parser, &open_documents).await;
        }
    }
}

/// Handle a single file system event by updating the project graph and symbol index.
async fn handle_file_event(
    event: watcher::ProjectFileEvent,
    root: &Path,
    graph: &Arc<tokio::sync::RwLock<Option<ProjectGraph>>>,
    index: &Arc<tokio::sync::RwLock<crate::symbol_index::SymbolIndex>>,
    parser: &Arc<std::sync::Mutex<GDScriptParser>>,
    open_documents: &Arc<tokio::sync::RwLock<HashMap<Url, String>>>,
) {
    match event {
        watcher::ProjectFileEvent::ShaderChanged(path) => {
            if let Ok(uri) = Url::from_file_path(&path) {
                if open_documents.read().await.contains_key(&uri) {
                    return; // In-memory edits are fresher than disk
                }
                if let Some(content) = read_text_file_limited(&path) {
                    let tree = {
                        let mut shader_p = gozen_parser::GDShaderParser::new();
                        shader_p.parse(&content)
                    };
                    if let Some(tree) = tree {
                        let mut idx = index.write().await;
                        idx.index_shader_file(&uri, &content, &tree);
                    }
                }
            }
        }
        watcher::ProjectFileEvent::ScriptChanged(path) => {
            let is_open = if let Ok(uri) = Url::from_file_path(&path) {
                open_documents.read().await.contains_key(&uri)
            } else {
                false
            };
            if !is_open {
                if let Some(content) = read_text_file_limited(&path) {
                    let res_path = to_res_path(&path, root);
                    let tree = {
                        let mut p = parser.lock().unwrap_or_else(|e| e.into_inner());
                        p.parse(&content)
                    };
                    {
                        let mut g = graph.write().await;
                        if let Some(g) = g.as_mut() {
                            g.update_script(&res_path, &content);
                        }
                    }
                    if let Some(tree) = tree {
                        if let Ok(uri) = Url::from_file_path(&path) {
                            let mut idx = index.write().await;
                            idx.index_file(&uri, &content, &tree);
                        }
                    }
                }
            }
        }
        watcher::ProjectFileEvent::SceneChanged(path) => {
            if let Some(content) = read_text_file_limited(&path) {
                let res_path = to_res_path(&path, root);
                let mut g = graph.write().await;
                if let Some(g) = g.as_mut() {
                    let _ = g.update_scene(&res_path, &content);
                }
            }
        }
        watcher::ProjectFileEvent::ResourceChanged(path) => {
            if let Some(content) = read_text_file_limited(&path) {
                let res_path = to_res_path(&path, root);
                let mut g = graph.write().await;
                if let Some(g) = g.as_mut() {
                    g.update_resource(&res_path, &content);
                }
            }
        }
        watcher::ProjectFileEvent::ProjectSettingsChanged => {
            if let Ok(new_graph) = ProjectGraph::build(root) {
                let mut g = graph.write().await;
                *g = Some(new_graph);
            }
        }
        watcher::ProjectFileEvent::FileRemoved(path) => {
            let res_path = to_res_path(&path, root);
            let mut g = graph.write().await;
            if let Some(g) = g.as_mut() {
                g.remove_file(&res_path);
            }
            if let Ok(uri) = Url::from_file_path(&path) {
                let mut idx = index.write().await;
                idx.remove_uri(&uri);
            }
        }
    }
}

pub async fn run_stdio(config: GozenConfig) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) =
        LspService::new(move |client| GozenLspBackend::new(client, config.clone()));
    Server::new(stdin, stdout, socket).serve(service).await;
}
