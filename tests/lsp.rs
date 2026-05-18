// tests/lsp.rs — LSP integration (tower-lsp via `pblang lsp`)
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn lsp_frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn lsp_did_open_json(uri: &str, text: &str) -> String {
    let t = serde_json::to_string(text).expect("text json");
    format!(
        r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","languageId":"pblang","version":1,"text":{t}}}}}}}"#
    )
}

fn pblang_bin() -> &'static str {
    env!("CARGO_BIN_EXE_pblang")
}

/// Read one LSP message body from a byte stream (after headers). Returns None on EOF.
fn read_lsp_body<R: Read>(reader: &mut R, buf: &mut Vec<u8>) -> Option<String> {
    buf.clear();
    loop {
        let mut byte = [0u8; 1];
        if reader.read_exact(&mut byte).is_err() {
            return None;
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            panic!("LSP header too large");
        }
    }
    let header = String::from_utf8_lossy(buf);
    let mut content_len = None;
    for line in header.lines() {
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            content_len = rest.trim().parse().ok();
        }
    }
    let len = content_len.expect("Content-Length header");
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).ok()?;
    Some(String::from_utf8(body).expect("utf8 body"))
}

/// Send LSP frames; for requests with `id`, wait until that id appears in stdout.
fn run_lsp_exchange(steps: &[(&str, Option<u32>)]) -> String {
    let mut child = Command::new(pblang_bin())
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn pblang lsp");

    let mut stdout_pipe = child.stdout.take().expect("stdout");
    let mut stdin = child.stdin.take().expect("stdin");

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut collected = String::new();
        while let Some(msg) = read_lsp_body(&mut stdout_pipe, &mut buf) {
            collected.push_str(&msg);
            let _ = tx.send(collected.clone());
        }
    });

    let mut all_stdout = String::new();
    for (body, wait_id) in steps {
        stdin.write_all(lsp_frame(body).as_bytes()).unwrap();
        stdin.flush().ok();
        if let Some(id) = wait_id {
            let needle = format!("\"id\":{id}");
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Ok(s) = rx.try_recv() {
                    all_stdout = s;
                }
                if all_stdout.contains(&needle)
                    && !all_stdout.contains(&format!("\"id\":{id},\"error\""))
                {
                    // accept success responses; errors still count as received
                    if all_stdout.contains(&format!("\"id\":{id}")) {
                        break;
                    }
                }
                if all_stdout.contains(&needle) {
                    break;
                }
                if Instant::now() >= deadline {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        } else {
            thread::sleep(Duration::from_millis(20));
        }
    }

    drop(stdin);
    thread::sleep(Duration::from_millis(100));
    let out = child.wait_with_output().expect("lsp process failed");
    let tail = String::from_utf8_lossy(&out.stdout);
    if all_stdout.len() < tail.len() {
        all_stdout = tail.into_owned();
    }
    all_stdout
}

#[test]
fn lsp_initialize_returns_completion_capability() {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let stdout = run_lsp_exchange(&[
        (init, Some(1)),
        (inited, None),
        (shutdown, Some(2)),
        (exit_n, None),
    ]);
    assert!(
        stdout.contains("completionProvider"),
        "expected completionProvider in initialize response, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains(&format!(r#""version":"{}""#, env!("CARGO_PKG_VERSION"))),
        "expected initialize response to include package version {}, got:\n{}",
        env!("CARGO_PKG_VERSION"),
        stdout
    );
}

#[test]
fn lsp_completion_returns_builtin_items() {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let comp = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///test.sapp"},"position":{"line":0,"character":0}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let stdout = run_lsp_exchange(&[
        (init, Some(1)),
        (inited, None),
        (comp, Some(2)),
        (shutdown, Some(3)),
        (exit_n, None),
    ]);
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
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///test.sapp","languageId":"pblang","version":1,"text":"fn my_custom_fn() { return 1; }\nlet local_var = 2;\n"}}}"#;
    let comp = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///test.sapp"},"position":{"line":0,"character":0}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let stdout = run_lsp_exchange(&[
        (init, Some(1)),
        (inited, None),
        (did_open, None),
        (comp, Some(2)),
        (shutdown, Some(3)),
        (exit_n, None),
    ]);
    assert!(
        stdout.contains("my_custom_fn"),
        "expected local function name in completion items, got:\n{}",
        stdout
    );
}

#[test]
fn lsp_hover_returns_builtin_detail() {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = lsp_did_open_json("file:///hover.sapp", "fn x() { print(1); }\n");
    let hover = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///hover.sapp"},"position":{"line":0,"character":9}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let stdout = run_lsp_exchange(&[
        (init, Some(1)),
        (inited, None),
        (&did_open, None),
        (hover, Some(2)),
        (shutdown, Some(3)),
        (exit_n, None),
    ]);
    assert!(
        (stdout.contains("print(value)") || stdout.contains("builtin")) && stdout.contains("print"),
        "expected hover signature for builtin print, got:\n{}",
        stdout
    );
}

#[test]
fn lsp_definition_returns_location() {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = lsp_did_open_json(
        "file:///def.sapp",
        "fn my_target() { return 1; }\nlet y = my_target();\n",
    );
    let def = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///def.sapp"},"position":{"line":1,"character":9}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let stdout = run_lsp_exchange(&[
        (init, Some(1)),
        (inited, None),
        (&did_open, None),
        (def, Some(2)),
        (shutdown, Some(3)),
        (exit_n, None),
    ]);
    assert!(
        stdout.contains("file:///def.sapp") && stdout.contains("\"line\":0"),
        "expected definition Location on declaration line 0, got:\n{}",
        stdout
    );
}

#[test]
fn lsp_completion_reflects_did_change_latest_text() {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let did_open = lsp_did_open_json(
        "file:///change.sapp",
        "fn old_name() { return 1; }\nlet y = old_name();\n",
    );
    let did_change = r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///change.sapp","version":2},"contentChanges":[{"text":"fn new_name() { return 2; }\nlet y = new_name();\n"}]}}"#;
    let comp = r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///change.sapp"},"position":{"line":1,"character":12}}}"#;
    let shutdown = r#"{"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}"#;
    let exit_n = r#"{"jsonrpc":"2.0","method":"exit","params":null}"#;

    let stdout = run_lsp_exchange(&[
        (init, Some(1)),
        (inited, None),
        (&did_open, None),
        (did_change, None),
        (comp, Some(2)),
        (shutdown, Some(3)),
        (exit_n, None),
    ]);

    assert!(
        stdout.contains("new_name"),
        "expected completion to use didChange latest document text, got:\n{}",
        stdout
    );
}
