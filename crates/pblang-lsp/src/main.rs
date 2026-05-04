use chumsky::error::SimpleReason;
use chumsky::prelude::Simple;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

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
        SimpleReason::Unclosed { span: _, delimiter } => format!("unclosed delimiter '{delimiter}'"),
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

impl Backend {
    async fn publish_diagnostics_for(&self, uri: Url, src: String) {
        let mut diags: Vec<Diagnostic> = vec![];

        match pblang_core::parse_with_polyglot(&src) {
            Ok((stmts, _blocks)) => {
                let tdiags = pblang_typeck::check(&stmts);
                diags.extend(tdiags.into_iter().map(|d| diag_from_typeck(&src, d)));
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
                definition_provider: Some(OneOf::Left(false)),
                completion_provider: None,
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
            .log_message(MessageType::INFO, "Sapphire LSP initialized")
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
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: Arc::new(Mutex::new(HashMap::new())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}

