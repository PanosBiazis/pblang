//! Differential test for `stdlib/compiler/formatter.sapp` vs. the Rust
//! `pblang fmt` CLI. Phase 0 of the Rust→pblang migration (see
//! `docs/Bootstrap_safety.md` Layer 2 and `docs/PrjAnalysis.md` §9).
//!
//! Each snippet in `INDENT_CORPUS` is pre-normalized for operator and
//! comma spacing (the pblang formatter only handles indentation today
//! — see the module header of `stdlib/compiler/formatter.sapp`). For
//! each snippet we:
//!
//!   1. Write the mangled input to a temp file and run `pblang fmt`,
//!      capturing the Rust-formatted output.
//!   2. Run the pblang-side `format_source` on the same mangled input
//!      via the VM and capture its output.
//!
//! The two outputs must match byte-for-byte. Any divergence means one
//! implementation has drifted from the other and the offending
//! snippet's label tells you which construct regressed.

mod common;

use common::run_collect;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn pblang_bin() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_pblang") {
        return std::path::PathBuf::from(p);
    }
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("debug");
    p.push(if cfg!(windows) {
        "pblang.exe"
    } else {
        "pblang"
    });
    p
}

/// `(label, mangled input)`. Each input is intentionally mis-formatted
/// so the formatter has something to fix.
const INDENT_CORPUS: &[(&str, &str)] = &[
    ("single_fn", "fn main() {\nlet x = 1;\nprint x;\n}\n"),
    (
        "nested_blocks",
        "fn main() {\nlet x = 1;\nif x == 1 {\nprint x;\n}\n}\n",
    ),
    (
        "two_top_level_fns",
        "fn a() {\nlet x = 1;\n}\nfn b() {\nlet y = 2;\n}\n",
    ),
    (
        "string_with_braces_inside",
        "fn main() {\nlet s = \"{not real braces}\";\nprint s;\n}\n",
    ),
    // Operator-spacing and comma-spacing coverage. Inputs use no
    // surrounding spaces so the normalizer has work to do.
    (
        "binary_arith_no_spaces",
        "fn main() {\nlet x=1+2*3;\nprint x;\n}\n",
    ),
    (
        "compare_ops_no_spaces",
        "fn main() {\nlet b=1==2;\nlet c=1!=2;\nprint b;\n}\n",
    ),
    ("comma_in_call", "fn main() {\nfoo(1,2,3);\n}\n"),
    (
        "unary_minus_literal",
        "fn main() {\nlet n=-1;\nprint n;\n}\n",
    ),
    (
        "comma_inside_string_unchanged",
        "fn main() {\nlet s = \"a,b,c\";\nprint s;\n}\n",
    ),
    // NOTE: snippets containing `//`-line-comments are intentionally
    // excluded from this corpus. Both formatters' operator-spacing
    // passes rewrite `//` into `/ /` (they see two `/` operator chars
    // and insert a space). Until the formatters learn that `//`
    // begins a line comment, snippets with trailing `// ...` would
    // round-trip as `/ / ...` — semantically a comment rewrite the
    // user didn't ask for. Leave it out of the drift corpus.
];

fn run_rust_fmt(mangled: &str, max_line_length: Option<usize>) -> String {
    let bin = pblang_bin();
    assert!(
        bin.exists(),
        "pblang binary not found at {}; run `cargo build --bin pblang` first.",
        bin.display()
    );

    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_dir =
        std::env::temp_dir().join(format!("pblang-fmt-drift-{}-{}", std::process::id(), seq,));
    std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
    let path = tmp_dir.join("input.sapp");
    std::fs::write(&path, mangled).expect("write fmt input");

    let mut cmd = Command::new(&bin);
    cmd.arg("fmt");
    if let Some(cap) = max_line_length {
        cmd.arg(format!("--max-line-length={cap}"));
    }
    cmd.arg(&path);
    let output = cmd.output().expect("spawn pblang fmt");
    assert!(
        output.status.success(),
        "pblang fmt failed (stderr: {})",
        String::from_utf8_lossy(&output.stderr)
    );

    let formatted = std::fs::read_to_string(&path).expect("read formatted output");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    formatted
}

fn run_pblang_fmt(mangled: &str, max_line_length: Option<usize>) -> String {
    let escaped = pblang_string_escape(mangled);
    let cap_arg = match max_line_length {
        Some(n) => n.to_string(),
        None => "nil".to_string(),
    };
    let driver = format!(
        r#"
        let f = import("stdlib:compiler/formatter.sapp");
        print f.format_source("{escaped}", {cap_arg});
        "#
    );
    let out = run_collect(&driver);
    out.last()
        .and_then(|v| v.as_str().ok().map(String::from))
        .unwrap_or_default()
}

#[test]
fn fmt_drift_indent_corpus_agrees() {
    for (label, mangled) in INDENT_CORPUS {
        let rust_out = run_rust_fmt(mangled, None);
        let pblang_out = run_pblang_fmt(mangled, None);

        let rust_norm = rust_out.trim_end_matches('\n').to_string();
        let pblang_norm = pblang_out.trim_end_matches('\n').to_string();

        assert_eq!(
            rust_norm, pblang_norm,
            "fmt drift on snippet {label:?}:\n--- Rust output ---\n{rust_norm}\n\
             --- pblang output ---\n{pblang_norm}\n\
             --- input ---\n{mangled}\n\
             One of the two formatters has changed without the other being \
             updated. See docs/Bootstrap_safety.md Layer 2."
        );
    }
}

/// `(label, mangled input, max_line_length)`. Each input is at least
/// one line longer than the cap, forcing the word-wrap path.
const WRAP_CORPUS: &[(&str, &str, usize)] = &[
    (
        "wrap_long_arith",
        "fn main() {\nlet result = first + second + third + fourth + fifth;\n}\n",
        30,
    ),
    (
        "wrap_long_call_args",
        "fn main() {\nfoo(alpha, beta, gamma, delta, epsilon, zeta);\n}\n",
        25,
    ),
    (
        "wrap_no_spaces_in_window",
        "fn main() {\nlet n=verylongidentifierwithnowhitespaceatall+1;\n}\n",
        20,
    ),
];

#[test]
fn fmt_drift_wrap_corpus_agrees() {
    for (label, mangled, cap) in WRAP_CORPUS {
        let rust_out = run_rust_fmt(mangled, Some(*cap));
        let pblang_out = run_pblang_fmt(mangled, Some(*cap));

        let rust_norm = rust_out.trim_end_matches('\n').to_string();
        let pblang_norm = pblang_out.trim_end_matches('\n').to_string();

        assert_eq!(
            rust_norm, pblang_norm,
            "fmt-wrap drift on snippet {label:?} (cap={cap}):\n--- Rust output ---\n{rust_norm}\n\
             --- pblang output ---\n{pblang_norm}\n\
             --- input ---\n{mangled}\n\
             One of the two formatters has changed its word-wrap behavior \
             without the other being updated. See docs/Bootstrap_safety.md \
             Layer 2."
        );
    }
}

fn pblang_string_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}
