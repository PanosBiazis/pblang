//! `pblang-lsp` binary — same server as `pblang lsp`.

#[tokio::main]
async fn main() {
    pblang_lsp::run_stdio_server().await;
}
