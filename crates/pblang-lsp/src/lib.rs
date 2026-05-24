//! pblang Language Server (tower-lsp): parse + typecheck diagnostics, hover, goto-def,
//! completions, document symbols, workspace symbols, and code actions.

use pblang_core::{ParseError, Span as PbSpan, Stmt, StmtKind};
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

fn fmt_parse_error(err: &ParseError) -> String {
    // Chumsky 1.0's `Simple` no longer exposes the rich label/expected/reason
    // accessors that 0.9 did; we now ship the message verbatim from the
    // pblang-core conversion.
    err.message.clone()
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

fn diag_from_parse(src: &str, e: &ParseError) -> Diagnostic {
    Diagnostic {
        range: range_from_span(src, e.span.start, e.span.end),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("pblang-parser".into()),
        message: fmt_parse_error(e),
        ..Diagnostic::default()
    }
}

fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn utf16_col_to_byte(line: &str, col_utf16: u32) -> usize {
    let target = col_utf16 as usize;
    let mut utf16_seen = 0usize;
    for (idx, ch) in line.char_indices() {
        if utf16_seen >= target {
            return idx;
        }
        utf16_seen += ch.len_utf16();
    }
    line.len()
}

fn line_at(src: &str, line_idx: usize) -> Option<&str> {
    src.split('\n').nth(line_idx)
}

fn char_at(line: &str, byte_idx: usize) -> Option<char> {
    line.get(byte_idx..)?.chars().next()
}

fn prev_char_start(line: &str, byte_idx: usize) -> Option<usize> {
    line.get(..byte_idx)?.char_indices().last().map(|(i, _)| i)
}

fn word_at_position(src: &str, pos: Position) -> Option<String> {
    let line = line_at(src, pos.line as usize)?;
    let mut byte = utf16_col_to_byte(line, pos.character).min(line.len());

    // If cursor is at token boundary/end, allow selecting previous identifier char.
    let probe = match char_at(line, byte) {
        Some(ch) if is_ident_char(ch) => byte,
        _ => {
            let prev = prev_char_start(line, byte)?;
            if is_ident_char(char_at(line, prev)?) {
                prev
            } else {
                return None;
            }
        }
    };

    let mut start = probe;
    while let Some(prev) = prev_char_start(line, start) {
        if let Some(ch) = char_at(line, prev) {
            if is_ident_char(ch) {
                start = prev;
                continue;
            }
        }
        break;
    }

    byte = probe;
    while let Some(ch) = char_at(line, byte) {
        if is_ident_char(ch) {
            byte += ch.len_utf8();
        } else {
            break;
        }
    }

    if start < byte {
        Some(line[start..byte].to_string())
    } else {
        None
    }
}

/// Try to find the string literal at the given position (for import paths).
fn string_at_position(src: &str, pos: Position) -> Option<String> {
    let line = line_at(src, pos.line as usize)?;
    let byte = utf16_col_to_byte(line, pos.character).min(line.len());

    // Search left for opening quote
    let mut start = byte;
    let mut found_quote = false;
    while start > 0 {
        start -= 1;
        if let Some(ch) = char_at(line, start) {
            if ch == '"' || ch == '\'' {
                found_quote = true;
                break;
            }
        }
    }
    if !found_quote {
        return None;
    }

    // Search right for closing quote
    let mut end = start + 1;
    while end < line.len() {
        if let Some(ch) = char_at(line, end) {
            if ch == '"' || ch == '\'' {
                return Some(line[start + 1..end].to_string());
            }
        }
        end += 1;
    }
    None
}

/// Map of known stdlib module paths to the functions they export.
fn stdlib_exports() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    m.insert(
        "stdlib:json.sapp",
        vec!["parse", "stringify", "validate", "pretty"],
    );
    m.insert(
        "stdlib:http.sapp",
        vec![
            "get",
            "post",
            "get_json",
            "post_json",
            "post_auth",
            "get_auth",
            "post_json_auth",
            "parse_sse_line",
            "retry",
            "build_query_string",
            "get_with_params",
            "parse_response_json",
        ],
    );
    m.insert(
        "stdlib:async.sapp",
        vec![
            "go",
            "await",
            "await_all",
            "await_race",
            "sleep",
            "timeout",
            "pool",
            "map_parallel",
            "debounce",
            "throttle",
        ],
    );
    m.insert(
        "stdlib:math.sapp",
        vec![
            "abs", "sqrt", "pow", "sin", "cos", "tan", "log", "exp", "floor", "ceil", "round",
        ],
    );
    m.insert(
        "stdlib:strings.sapp",
        vec![
            "trim",
            "split",
            "join",
            "replace",
            "to_upper",
            "to_lower",
            "contains",
            "starts_with",
            "ends_with",
        ],
    );
    m.insert(
        "stdlib:fs.sapp",
        vec![
            "read", "write", "append", "exists", "delete", "list_dir", "mkdir",
        ],
    );
    m.insert(
        "stdlib:dataframe.sapp",
        vec![
            "from_rows",
            "from_csv_string",
            "from_csv",
            "to_csv",
            "nrows",
            "ncols",
            "head",
            "tail",
            "select_columns",
            "filter",
            "map_rows",
            "sort_by",
            "column",
            "add_column",
            "group_by",
            "describe",
            "join",
            "unique",
            "value_counts",
        ],
    );
    m.insert(
        "stdlib:csv.sapp",
        vec!["parse", "stringify", "parse_file", "write_file"],
    );
    m.insert(
        "stdlib:time.sapp",
        vec!["now", "sleep_ms", "format", "parse_iso", "elapsed"],
    );
    m.insert(
        "stdlib:log.sapp",
        vec!["debug", "info", "warn", "error", "set_level"],
    );
    m.insert(
        "stdlib:testing.sapp",
        vec![
            "assert",
            "assert_eq",
            "assert_ne",
            "assert_throws",
            "run_tests",
        ],
    );
    m
}

/// Known functions from stdlib modules (module path -> function name).
fn find_stdlib_export(name: &str) -> Option<&'static str> {
    for (mod_path, exports) in stdlib_exports() {
        if exports.contains(&name) {
            return Some(mod_path);
        }
    }
    None
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

fn push_symbol(
    out: &mut Vec<SymbolEntry>,
    name: String,
    span: PbSpan,
    detail: String,
    kind: CompletionItemKind,
) {
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
            StmtKind::While(_, body) | StmtKind::Block(body) | StmtKind::Loop(body) => {
                collect_stmt_symbols(body, out);
            }
            StmtKind::Seq(stmts) => {
                collect_stmt_symbols(stmts, out);
            }
            _ => {}
        }
    }
}

/// Collect top-level document symbols from AST (not recursive into function bodies).
#[allow(deprecated)]
fn collect_document_symbols(stmts: &[Stmt], src: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::Fn(name, params, _ret, _body) => {
                let detail = format!(
                    "fn {}({})",
                    name,
                    params
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                symbols.push(DocumentSymbol {
                    name: name.clone(),
                    detail: Some(detail),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range: range_from_span(src, stmt.span.start, stmt.span.end),
                    selection_range: range_from_span(src, stmt.span.start, stmt.span.end),
                    children: None,
                });
            }
            StmtKind::Let(name, _, _) => {
                symbols.push(DocumentSymbol {
                    name: name.clone(),
                    detail: Some(format!("let {}", name)),
                    kind: SymbolKind::VARIABLE,
                    tags: None,
                    deprecated: None,
                    range: range_from_span(src, stmt.span.start, stmt.span.end),
                    selection_range: range_from_span(src, stmt.span.start, stmt.span.end),
                    children: None,
                });
            }
            StmtKind::Const(name, _, _) => {
                symbols.push(DocumentSymbol {
                    name: name.clone(),
                    detail: Some(format!("const {}", name)),
                    kind: SymbolKind::CONSTANT,
                    tags: None,
                    deprecated: None,
                    range: range_from_span(src, stmt.span.start, stmt.span.end),
                    selection_range: range_from_span(src, stmt.span.start, stmt.span.end),
                    children: None,
                });
            }
            StmtKind::Export(names) => {
                // Group exports as a module-like symbol
                let children: Vec<DocumentSymbol> = names
                    .iter()
                    .map(|n| DocumentSymbol {
                        name: n.clone(),
                        detail: Some("export".into()),
                        kind: SymbolKind::PROPERTY,
                        tags: None,
                        deprecated: None,
                        range: range_from_span(src, stmt.span.start, stmt.span.end),
                        selection_range: range_from_span(src, stmt.span.start, stmt.span.end),
                        children: None,
                    })
                    .collect();
                symbols.push(DocumentSymbol {
                    name: "exports".into(),
                    detail: Some(format!("{} exports", names.len())),
                    kind: SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    range: range_from_span(src, stmt.span.start, stmt.span.end),
                    selection_range: range_from_span(src, stmt.span.start, stmt.span.end),
                    children: Some(children),
                });
            }
            _ => {}
        }
    }
    symbols
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
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "pblang-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "pblang LSP — diagnostics (parse+typeck), hover, goto-def, completions, doc symbols, workspace symbols, code actions",
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

        // Check stdlib exports
        if let Some(mod_path) = find_stdlib_export(&word) {
            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "{} (from {})",
                    word, mod_path
                ))),
                range: None,
            }));
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

        // First, check if cursor is on a string literal (import path navigation)
        let line = line_at(&src, pos.line as usize).unwrap_or("");
        // Check for import statement context
        let line_trimmed = line.trim();
        if line_trimmed.starts_with("import") {
            if let Some(path) = string_at_position(&src, pos) {
                // Resolve import path to a file URL
                let resolved = resolve_import_path(&uri, &path);
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: resolved,
                    range: Range {
                        start: Position::new(0, 0),
                        end: Position::new(0, 1),
                    },
                })));
            }
        }

        let word = match word_at_position(&src, pos) {
            Some(w) => w,
            None => return Ok(None),
        };

        // Check builtins (no goto)
        for &(name, _) in BUILTINS {
            if name == word {
                return Ok(None);
            }
        }

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

        // Add known stdlib exports
        let mut seen_exports: HashSet<String> = HashSet::new();
        for (_mod_path, exports) in stdlib_exports() {
            for name in exports {
                if seen_exports.insert(name.to_string()) {
                    items.push(CompletionItem {
                        label: name.to_string(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(format!("(stdlib: {})", name)),
                        ..CompletionItem::default()
                    });
                }
            }
        }

        for kw in &[
            "let", "const", "fn", "return", "if", "else", "while", "for", "in", "match", "case",
            "import", "export", "true", "false", "nil", "and", "or", "not", "try", "catch",
            "defer", "loop", "break", "continue", "throw",
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
                    StmtKind::Fn(name, fn_params, _ret, _body) => {
                        if !params.query.is_empty() && !name.to_lowercase().contains(&query_lower) {
                            continue;
                        }
                        let detail = format!(
                            "fn {}({})",
                            name,
                            fn_params
                                .iter()
                                .map(|p| p.name.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        symbols.push(SymbolInformation {
                            name: name.clone(),
                            kind: SymbolKind::FUNCTION,
                            tags: None,
                            deprecated: None,
                            location: Location {
                                uri: uri.clone(),
                                range: range_from_span(src, stmt.span.start, stmt.span.end),
                            },
                            container_name: Some(detail),
                        });
                    }
                    StmtKind::Let(name, _, _) => {
                        if !params.query.is_empty() && !name.to_lowercase().contains(&query_lower) {
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
                    StmtKind::Const(name, _, _) => {
                        if !params.query.is_empty() && !name.to_lowercase().contains(&query_lower) {
                            continue;
                        }
                        symbols.push(SymbolInformation {
                            name: name.clone(),
                            kind: SymbolKind::CONSTANT,
                            tags: None,
                            deprecated: None,
                            location: Location {
                                uri: uri.clone(),
                                range: range_from_span(src, stmt.span.start, stmt.span.end),
                            },
                            container_name: None,
                        });
                    }
                    StmtKind::Export(names) => {
                        for n in names {
                            if !params.query.is_empty() && !n.to_lowercase().contains(&query_lower)
                            {
                                continue;
                            }
                            symbols.push(SymbolInformation {
                                name: n.clone(),
                                kind: SymbolKind::PROPERTY,
                                tags: None,
                                deprecated: None,
                                location: Location {
                                    uri: uri.clone(),
                                    range: range_from_span(src, stmt.span.start, stmt.span.end),
                                },
                                container_name: Some("export".into()),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(Some(symbols))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let src = {
            let docs = self.docs.lock().await;
            docs.get(&uri).cloned().unwrap_or_default()
        };
        let Ok((stmts, _)) = pblang_core::parse_with_polyglot(&src) else {
            return Ok(None);
        };

        let symbols = collect_document_symbols(&stmts, &src);
        // Return as DocumentSymbol (hierarchical) variant
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let src = {
            let docs = self.docs.lock().await;
            docs.get(&uri).cloned().unwrap_or_default()
        };

        let mut actions: Vec<CodeActionOrCommand> = Vec::new();

        // Process diagnostics for code actions
        for diag in &params.context.diagnostics {
            let msg = &diag.message;

            // "unknown variable" or "undefined" → suggest importing from stdlib
            if msg.contains("unknown") || msg.contains("undefined") || msg.contains("not defined") {
                // Try to extract the variable name from the diagnostic range
                let word = word_at_position(&src, diag.range.start);
                if let Some(name) = word {
                    if let Some(mod_path) = find_stdlib_export(&name) {
                        let edit = TextEdit {
                            range: Range {
                                start: Position::new(0, 0),
                                end: Position::new(0, 0),
                            },
                            new_text: format!("import \"{}\";\n", mod_path),
                        };
                        let changes = {
                            let mut m = HashMap::new();
                            m.insert(uri.clone(), vec![edit]);
                            m
                        };
                        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                            title: format!("Add import for '{}' from {}", name, mod_path),
                            kind: Some(CodeActionKind::QUICKFIX),
                            diagnostics: Some(vec![diag.clone()]),
                            edit: Some(WorkspaceEdit {
                                changes: Some(changes),
                                ..WorkspaceEdit::default()
                            }),
                            is_preferred: Some(true),
                            ..CodeAction::default()
                        }));
                    }
                }
            }

            // Type mismatch suggestions
            if msg.contains("type") && (msg.contains("mismatch") || msg.contains("expected")) {
                let action = CodeAction {
                    title: "[pblang] Review type annotation".into(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diag.clone()]),
                    ..CodeAction::default()
                };
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }
}

/// Attempt to resolve an import path to a file URL.
fn resolve_import_path(current_uri: &Url, import_path: &str) -> Url {
    // Handle stdlib: prefix → look relative to the project
    if import_path.starts_with("stdlib:") {
        // We can't guess the exact file location without knowing the project root,
        // but we try to construct a path relative to the current file.
        if let Ok(current_path) = current_uri.to_file_path() {
            let relative = import_path.strip_prefix("stdlib:").unwrap_or(import_path);
            if let Some(parent) = current_path.parent() {
                let resolved = parent.join("..").join("stdlib").join(relative);
                return Url::from_file_path(resolved).unwrap_or_else(|_| current_uri.clone());
            }
        }
    }

    // Try relative to current file
    if let Ok(current_path) = current_uri.to_file_path() {
        if let Some(parent) = current_path.parent() {
            let resolved = parent.join(import_path);
            return Url::from_file_path(resolved).unwrap_or_else(|_| current_uri.clone());
        }
    }

    current_uri.clone()
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
        assert!(diags
            .iter()
            .all(|d| d.source == "pblang-typeck" || d.source == "pblang-parser"));
    }

    #[test]
    fn document_symbols_basic() {
        let src = "fn add(a, b) { return a + b; }\nlet x = 42;\nconst PI = 3.14;";
        let (stmts, _) = pblang_core::parse_with_polyglot(src).unwrap();
        let syms = collect_document_symbols(&stmts, src);
        assert_eq!(syms.len(), 3);
        assert!(syms
            .iter()
            .any(|s| s.name == "add" && s.kind == SymbolKind::FUNCTION));
        assert!(syms
            .iter()
            .any(|s| s.name == "x" && s.kind == SymbolKind::VARIABLE));
        assert!(syms
            .iter()
            .any(|s| s.name == "PI" && s.kind == SymbolKind::CONSTANT));
    }

    #[test]
    fn stdlib_exports_lookup() {
        assert_eq!(find_stdlib_export("get_json"), Some("stdlib:http.sapp"));
        assert_eq!(find_stdlib_export("go"), Some("stdlib:async.sapp"));
        assert_eq!(find_stdlib_export("nonexistent"), None);
    }
}
