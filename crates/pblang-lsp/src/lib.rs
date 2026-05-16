//! pblang Language Server (tower-lsp): parse + typecheck diagnostics, hover, goto-def, completions.

use chumsky::error::SimpleReason;
use chumsky::prelude::Simple;
use pblang_core::{Span as PbSpan, Stmt, StmtKind};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

/// Summary of a diagnostic for tests and tooling (not full LSP types).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSummary {
    pub source: String,
    pub severity: String,
    pub message: String,
}

/// Collect parse and typecheck diagnostics for a source buffer (no LSP client).
pub fn collect_diagnostics(src: &str) -> Vec<DiagnosticSummary> {
    let mut out = Vec::new();
    match pblang_core::parse_with_polyglot(src) {
        Ok((stmts, _)) => {
            for d in pblang_typeck::check(&stmts) {
                out.push(DiagnosticSummary {
                    source: "pblang-typeck".into(),
                    severity: "warning".into(),
                    message: d.message,
                });
            }
        }
        Err(perrs) => {
            for e in perrs.iter().take(32) {
                out.push(DiagnosticSummary {
                    source: "pblang-parser".into(),
                    severity: "error".into(),
                    message: fmt_parse_error(e),
                });
            }
        }
    }
    out
}

/// Run the LSP server on stdin/stdout (used by `pblang lsp` and `pblang-lsp` binary).
pub async fn run_stdio_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: Arc::new(Mutex::new(HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[derive(Clone)]
struct Backend {
    client: Client,
    docs: Arc<Mutex<HashMap<Url, String>>>,
}

fn position_of_byte(src: &str, byte_idx: usize) -> Position {
    let byte_idx = byte_idx.min(src.len());
    let mut line: u32 = 0;
    let mut line_start: usize = 0;
    for (i, ch) in src.char_indices() {
        if i >= byte_idx {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    let slice = &src[line_start..byte_idx];
    let col_utf16 = slice.encode_utf16().count() as u32;
    Position {
        line,
        character: col_utf16,
    }
}

fn range_from_span(src: &str, start: usize, end: usize) -> Range {
    let start = start.min(src.len());
    let mut end = end.min(src.len());
    if end <= start {
        end = (start + 1).min(src.len());
    }
    Range {
        start: position_of_byte(src, start),
        end: position_of_byte(src, end),
    }
}

fn fmt_parse_error(err: &Simple<char>) -> String {
    let label = err.label();
    let found = err.found();
    let mut expected = err.expected().copied().flatten().collect::<Vec<char>>();
    expected.sort();
    expected.dedup();
    let base = match err.reason() {
        SimpleReason::Unclosed {
            span: _,
            delimiter,
        } => format!("unclosed delimiter '{delimiter}'"),
        SimpleReason::Custom(msg) => msg.to_string(),
        SimpleReason::Unexpected => match (found, expected.is_empty()) {
            (Some(f), false) => format!("unexpected '{f}', expected one of: {:?}", expected),
            (Some(f), true) => format!("unexpected '{f}'"),
            (None, false) => format!("unexpected end of input, expected one of: {:?}", expected),
            (None, true) => "parse error".to_string(),
        },
    };
    if let Some(l) = label {
        format!("{base} (while parsing {l})")
    } else {
        base
    }
}

fn diag_from_typeck(src: &str, d: pblang_typeck::Diagnostic) -> Diagnostic {
    let range = if let Some(sp) = d.span {
        range_from_span(src, sp.start, sp.end)
    } else {
        Range {
            start: Position::new(0, 0),
            end: Position::new(0, 1),
        }
    };
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("pblang-typeck".into()),
        message: d.message,
        ..Diagnostic::default()
    }
}

fn diag_from_parse(src: &str, e: &Simple<char>) -> Diagnostic {
    let sp = e.span();
    Diagnostic {
        range: range_from_span(src, sp.start, sp.end),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("pblang-parser".into()),
        message: fmt_parse_error(e),
        ..Diagnostic::default()
    }
}

fn word_at_position(src: &str, pos: Position) -> Option<String> {
    let lines: Vec<&str> = src.split('\n').collect();
    let line_idx = pos.line as usize;
    if line_idx >= lines.len() {
        return None;
    }
    let line = lines[line_idx];
    let col = pos.character as usize;
    if col >= line.len() {
        return None;
    }

    let mut start = col;
    while start > 0 {
        let ch = line.chars().nth(start - 1).unwrap_or(' ');
        if ch.is_alphanumeric() || ch == '_' {
            start -= 1;
        } else {
            break;
        }
    }
    let mut end = col;
    while end < line.len() {
        let ch = line.chars().nth(end).unwrap_or(' ');
        if ch.is_alphanumeric() || ch == '_' {
            end += 1;
        } else {
            break;
        }
    }

    if start < end {
        Some(line[start..end].to_string())
    } else {
        None
    }
}

const BUILTINS: &[(&str, &str)] = &[
    ("print", "print(value) -> nil"),
    ("len", "len(value) -> number"),
    ("str", "str(value) -> string"),
    ("type", "type(value) -> string"),
    ("allocate", "allocate(value, is_mutable?) -> handle"),
    ("borrow_immutable", "borrow_immutable(handle) -> borrow_id"),
    ("borrow_mutable", "borrow_mutable(handle) -> borrow_id"),
    ("borrow_unique", "borrow_unique(handle) -> borrow_id"),
    ("load", "load(handle) -> value"),
    ("store", "store(handle, value) -> nil"),
    ("clock", "clock() -> number (seconds)"),
    ("now_ms", "now_ms() -> number (milliseconds)"),
    ("spawn", "spawn(closure) -> handle"),
    ("join", "join(handle) -> value"),
    ("channel", "channel() -> map{sender, receiver}"),
    ("json_parse", "json_parse(string) -> value"),
    ("json_stringify", "json_stringify(value) -> string"),
    ("http_get", "http_get(url) -> string"),
    ("http_post", "http_post(url, body) -> string"),
    ("fs_read", "fs_read(path) -> string"),
    ("import", "import(path) -> module"),
];

#[derive(Clone, Debug)]
struct SymbolEntry {
    name: String,
    span: PbSpan,
    detail: String,
    completion_kind: CompletionItemKind,
}

fn push_symbol(out: &mut Vec<SymbolEntry>, name: String, span: PbSpan, detail: String, kind: CompletionItemKind) {
    if !name.is_empty() {
        out.push(SymbolEntry {
            name,
            span,
            detail,
            completion_kind: kind,
        });
    }
}

fn collect_stmt_symbols(stmts: &[Stmt], out: &mut Vec<SymbolEntry>) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Fn(name, params, _ret, body) => {
                let ps: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                push_symbol(
                    out,
                    name.clone(),
                    stmt.span,
                    format!("fn {}({})", name, ps.join(", ")),
                    CompletionItemKind::FUNCTION,
                );
                for p in params {
                    push_symbol(
                        out,
                        p.name.clone(),
                        stmt.span,
                        format!("param {}", p.name),
                        CompletionItemKind::VARIABLE,
                    );
                }
                collect_stmt_symbols(body, out);
            }
            StmtKind::Let(name, _, _) | StmtKind::Const(name, _, _) => {
                push_symbol(
                    out,
                    name.clone(),
                    stmt.span,
                    format!("let {}", name),
                    CompletionItemKind::VARIABLE,
                );
            }
            StmtKind::For(name, _, body) | StmtKind::ForIn(name, _, body) => {
                push_symbol(
                    out,
                    name.clone(),
                    stmt.span,
                    format!("loop variable {}", name),
                    CompletionItemKind::VARIABLE,
                );
                collect_stmt_symbols(body, out);
            }
            StmtKind::ForInKV(k, v, _, body) => {
                push_symbol(
                    out,
                    k.clone(),
                    stmt.span,
                    format!("loop key {}", k),
                    CompletionItemKind::VARIABLE,
                );
                push_symbol(
                    out,
                    v.clone(),
                    stmt.span,
                    format!("loop value {}", v),
                    CompletionItemKind::VARIABLE,
                );
                collect_stmt_symbols(body, out);
            }
            StmtKind::If(_, then_body, else_body) => {
                collect_stmt_symbols(then_body, out);
                if let Some(else_body) = else_body {
                    collect_stmt_symbols(else_body, out);
                }
            }
            StmtKind::While(_, body) | StmtKind::Block(body) => {
                collect_stmt_symbols(body, out);
            }
            _ => {}
        }
    }
}

impl Backend {
    async fn publish_diagnostics_for(&self, uri: Url, src: String) {
        let mut diags: Vec<Diagnostic> = vec![];
        match pblang_core::parse_with_polyglot(&src) {
            Ok((stmts, _)) => {
                diags.extend(
                    pblang_typeck::check(&stmts)
                        .into_iter()
                        .map(|d| diag_from_typeck(&src, d)),
                );
            }
            Err(perrs) => {
                for e in perrs.iter().take(32) {
                    diags.push(diag_from_parse(&src, e));
                }
            }
        }
        self.client.publish_diagnostics(uri, diags, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "pblang-lsp".into(),
                version: Some("0.5.7".into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "pblang LSP — diagnostics (parse+typeck), hover, goto-def, completions, workspace symbols",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.docs.lock().await.insert(uri.clone(), text.clone());
        self.publish_diagnostics_for(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            self.docs.lock().await.insert(uri.clone(), text.clone());
            self.publish_diagnostics_for(uri, text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.docs.lock().await.remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let src = {
            let docs = self.docs.lock().await;
            docs.get(&uri).cloned().unwrap_or_default()
        };
        let word = match word_at_position(&src, pos) {
            Some(w) => w,
            None => return Ok(None),
        };

        for &(name, sig) in BUILTINS {
            if name == word {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(sig.to_string())),
                    range: None,
                }));
            }
        }

        if let Ok((stmts, _)) = pblang_core::parse_with_polyglot(&src) {
            let mut symbols = Vec::new();
            collect_stmt_symbols(&stmts, &mut symbols);
            if let Some(sym) = symbols.iter().rev().find(|s| s.name == word) {
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(sym.detail.clone())),
                    range: Some(range_from_span(&src, sym.span.start, sym.span.end)),
                }));
            }
        }
        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let src = {
            let docs = self.docs.lock().await;
            docs.get(&uri).cloned().unwrap_or_default()
        };
        let word = match word_at_position(&src, pos) {
            Some(w) => w,
            None => return Ok(None),
        };

        if let Ok((stmts, _)) = pblang_core::parse_with_polyglot(&src) {
            let mut symbols = Vec::new();
            collect_stmt_symbols(&stmts, &mut symbols);
            if let Some(sym) = symbols.iter().rev().find(|s| s.name == word) {
                let range = range_from_span(&src, sym.span.start, sym.span.end);
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range,
                })));
            }
        }
        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let src = {
            let docs = self.docs.lock().await;
            docs.get(&uri).cloned().unwrap_or_default()
        };
        let mut items: Vec<CompletionItem> = Vec::new();

        for &(name, sig) in BUILTINS {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(sig.to_string()),
                ..CompletionItem::default()
            });
        }

        for kw in &[
            "let", "const", "fn", "return", "if", "else", "while", "for", "in", "match", "case",
            "import", "export", "true", "false", "nil", "and", "or", "not", "try", "catch", "defer",
        ] {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            });
        }

        if let Ok((stmts, _)) = pblang_core::parse_with_polyglot(&src) {
            let mut symbols = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            collect_stmt_symbols(&stmts, &mut symbols);
            for sym in symbols {
                if !seen.insert(sym.name.clone()) {
                    continue;
                }
                items.push(CompletionItem {
                    label: sym.name,
                    kind: Some(sym.completion_kind),
                    detail: Some(sym.detail),
                    ..CompletionItem::default()
                });
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    #[allow(deprecated)]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let query_lower = params.query.to_lowercase();
        let docs = self.docs.lock().await;
        let mut symbols = Vec::new();
        for (uri, src) in docs.iter() {
            let Ok((stmts, _)) = pblang_core::parse_with_polyglot(src) else {
                continue;
            };
            for stmt in &stmts {
                match &stmt.kind {
                    StmtKind::Fn(name, _, _, _) => {
                        if !params.query.is_empty()
                            && !name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        symbols.push(SymbolInformation {
                            name: name.clone(),
                            kind: SymbolKind::FUNCTION,
                            tags: None,
                            deprecated: None,
                            location: Location {
                                uri: uri.clone(),
                                range: range_from_span(src, stmt.span.start, stmt.span.end),
                            },
                            container_name: None,
                        });
                    }
                    StmtKind::Let(name, _, _) | StmtKind::Const(name, _, _) => {
                        if !params.query.is_empty()
                            && !name.to_lowercase().contains(&query_lower)
                        {
                            continue;
                        }
                        symbols.push(SymbolInformation {
                            name: name.clone(),
                            kind: SymbolKind::VARIABLE,
                            tags: None,
                            deprecated: None,
                            location: Location {
                                uri: uri.clone(),
                                range: range_from_span(src, stmt.span.start, stmt.span.end),
                            },
                            container_name: None,
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(Some(symbols))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_diagnostics_parse_error() {
        let diags = collect_diagnostics("fn broken( {");
        assert!(!diags.is_empty());
        assert_eq!(diags[0].source, "pblang-parser");
        assert_eq!(diags[0].severity, "error");
    }

    #[test]
    fn collect_diagnostics_valid_source() {
        let diags = collect_diagnostics("fn ok() { return 1; }");
        assert!(diags.iter().all(|d| d.source == "pblang-typeck" || d.source == "pblang-parser"));
    }
}
