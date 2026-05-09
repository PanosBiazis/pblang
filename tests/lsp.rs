// tests/lsp.rs — L2-delta: LSP stub initialize + completion
use std::io::Write;
use std::process::{Command, Stdio};

fn lsp_did_open_json(uri: &str, text: &str) -> String {
    let t = serde_json::to_string(text).expect("text json");
    format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","languageId":"pblang","version":1,"text":{t}}}}}}}"#
    )
}

fn pblang_bin() -> &'static str {
    env!("CARGO_BIN_EXE_pblang")
}

#[test]
fn lsp_initialize_returns_completion_capability() {
    let mut child = Command::new(pblang_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn sapphire lsp");

    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let msg = format!("Content-Length: {}\r\n\r\n{}", init_req.len(), init_req);
    let shutdown = r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#;
    let msg2 = format!("Content-Length: {}\r\n\r\n{}", shutdown.len(), shutdown);
    let exit_notif = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;
    let msg3 = format!("Content-Length: {}\r\n\r\n{}", exit_notif.len(), exit_notif);

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(msg.as_bytes()).unwrap();
        stdin.write_all(msg2.as_bytes()).unwrap();
        stdin.write_all(msg3.as_bytes()).unwrap();
    }

    let out = child.wait_with_output().expect("lsp process failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("completionProvider"),
        "expected completionProvider in initialize response, got:\n{}",
        stdout
    );
}

#[test]
fn lsp_completion_returns_builtin_items() {
    let mut child = Command::new(pblang_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn sapphire lsp");

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let comp = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///test.sapp"},"position":{"line":0,"character":0}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let full: String = [init, inited, comp, shutdown, exit_n]
        .iter()
        .map(|m| format!("Content-Length: {}\r\n\r\n{}", m.len(), m))
        .collect();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(full.as_bytes()).unwrap();
    }

    let out = child.wait_with_output().expect("lsp process failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"label\""),
        "expected completion items with label, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("print") || stdout.contains("import"),
        "expected builtin names in completions, got:\n{}",
        stdout
    );
}

#[test]
fn lsp_completion_includes_local_fn_after_did_open() {
    let mut child = Command::new(pblang_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn sapphire lsp");

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.sapp","languageId":"sapphire","version":1,"text":"fn my_custom_fn() { return 1; }\nlet local_var = 2;\n"}}}"#;
    let comp = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///test.sapp"},"position":{"line":0,"character":0}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let full: String = [init, inited, did_open, comp, shutdown, exit_n]
        .iter()
        .map(|m| format!("Content-Length: {}\r\n\r\n{}", m.len(), m))
        .collect();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(full.as_bytes()).unwrap();
    }

    let out = child.wait_with_output().expect("lsp process failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("my_custom_fn"),
        "expected local function name in completion items, got:\n{}",
        stdout
    );
}

#[test]
fn lsp_hover_returns_builtin_detail() {
    let mut child = Command::new(pblang_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn pblang lsp");

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = lsp_did_open_json("file:///hover.sapp", "fn x() { print(1); }\n");
    let hover = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///hover.sapp"},"position":{"line":0,"character":9}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let full: String = [
        init.to_string(),
        inited.to_string(),
        did_open,
        hover.to_string(),
        shutdown.to_string(),
        exit_n.to_string(),
    ]
    .iter()
    .map(|m| format!("Content-Length: {}\r\n\r\n{}", m.len(), m))
    .collect();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(full.as_bytes()).unwrap();
    }

    let out = child.wait_with_output().expect("lsp process failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("builtin") && stdout.contains("print"),
        "expected hover markdown for builtin print, got:\n{}",
        stdout
    );
}

#[test]
fn lsp_definition_returns_location() {
    let mut child = Command::new(pblang_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn pblang lsp");

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = lsp_did_open_json(
        "file:///def.sapp",
        "fn my_target() { return 1; }\nlet y = my_target();\n",
    );
    let def = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///def.sapp"},"position":{"line":1,"character":9}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let full: String = [
        init.to_string(),
        inited.to_string(),
        did_open,
        def.to_string(),
        shutdown.to_string(),
        exit_n.to_string(),
    ]
    .iter()
    .map(|m| format!("Content-Length: {}\r\n\r\n{}", m.len(), m))
    .collect();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(full.as_bytes()).unwrap();
    }

    let out = child.wait_with_output().expect("lsp process failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("file:///def.sapp") && stdout.contains("\"line\":0"),
        "expected definition Location on declaration line 0, got:\n{}",
        stdout
    );
}
