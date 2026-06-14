//! `pblang --doc` command handler.
//!
//! Generates text or HTML module docs for exports, top-level functions, and
//! polyglot blocks. This is a behavior-preserving extraction from `main.rs`.

use pblang_core::{parse_with_polyglot, StmtKind, TypeExpr, TypeKind};

use super::diag::print_parse_errors;

fn render_ty(t: &TypeExpr) -> String {
    match &t.kind {
        TypeKind::Any => "any".into(),
        TypeKind::Number => "number".into(),
        TypeKind::BigInt => "bigint".into(),
        TypeKind::Rational => "rational".into(),
        TypeKind::Decimal => "decimal".into(),
        TypeKind::Bool => "bool".into(),
        TypeKind::String => "string".into(),
        TypeKind::Nil => "nil".into(),
        TypeKind::Array(inner) => format!("array<{}>", render_ty(inner)),
        TypeKind::Map(k, v) => format!("map<{}, {}>", render_ty(k), render_ty(v)),
        TypeKind::Option(inner) => format!("{}?", render_ty(inner)),
        TypeKind::Result(ok, err) => {
            format!("result<{}, {}>", render_ty(ok), render_ty(err))
        }
        TypeKind::Generic(name) => name.clone(),
        TypeKind::NonNull(inner) => format!("{}!", render_ty(inner)),
    }
}

fn render_polyglot_attrs(attrs: &std::collections::HashMap<String, String>) -> String {
    if attrs.is_empty() {
        return "(none)".into();
    }
    let mut pairs = attrs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>();
    pairs.sort();
    pairs.join(", ")
}

fn polyglot_code_line_count(code: &str) -> usize {
    if code.is_empty() {
        0
    } else {
        code.lines().count()
    }
}

pub(crate) fn handle_doc(args: Vec<String>) {
    let Some(path) = args.get(1) else {
        eprintln!("usage: pblang --doc <file> [--html] [out_dir]");
        std::process::exit(2);
    };
    let html_mode = args.get(2).map(|s| s.as_str()) == Some("--html");
    let out_dir = if html_mode {
        args.get(3).map(String::as_str).unwrap_or("docs")
    } else {
        ""
    };
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("pblang --doc: failed to read '{}': {}", path, e);
            std::process::exit(1);
        }
    };

    match parse_with_polyglot(&src) {
        Ok((stmts, blocks)) => {
            let mut exports: Vec<String> = vec![];
            let mut fns: Vec<String> = vec![];

            for s in &stmts {
                match &s.kind {
                    StmtKind::Export(names) => {
                        exports.extend(names.iter().cloned());
                    }
                    StmtKind::Fn(name, params, ret, _body) => {
                        let ps = params
                            .iter()
                            .map(|p| match &p.ty {
                                Some(t) => format!("{}: {}", p.name, render_ty(t)),
                                None => p.name.clone(),
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let r = ret.as_ref().map(render_ty);
                        let sig = if let Some(r) = r {
                            format!("fn {name}({ps}) -> {r}")
                        } else {
                            format!("fn {name}({ps})")
                        };
                        fns.push(sig);
                    }
                    _ => {}
                }
            }

            exports.sort();
            exports.dedup();
            fns.sort();

            if html_mode {
                write_html_docs(path, out_dir, &exports, &fns, blocks);
            } else {
                print_text_docs(path, &exports, &fns, blocks);
            }
        }
        Err(errs) => {
            print_parse_errors(&src, errs, Some(path));
            std::process::exit(2);
        }
    }
}

fn print_text_docs(
    path: &str,
    exports: &[String],
    fns: &[String],
    blocks: Vec<pblang_core::PolyglotBlock>,
) {
    println!("## Module docs");
    println!();
    println!("- **file**: `{path}`");
    println!();
    println!("### Exports");
    if exports.is_empty() {
        println!();
        println!("(none)");
    } else {
        for e in exports {
            println!("- `{e}`");
        }
    }
    println!();
    println!("### Top-level functions");
    if fns.is_empty() {
        println!();
        println!("(none)");
    } else {
        for f in fns {
            println!("- `{f}`");
        }
    }
    println!();
    println!("### Polyglot blocks");
    if blocks.is_empty() {
        println!();
        println!("(none)");
    } else {
        for b in blocks {
            let name = b.name.as_deref().unwrap_or("<unnamed>");
            let block_args = b.args.as_deref().unwrap_or("");
            let attrs = render_polyglot_attrs(&b.attrs);
            let lines = polyglot_code_line_count(&b.code);
            println!(
                "- `@{} {}` `{}` (span {}..{}, attrs: {}, code lines: {})",
                b.lang, name, block_args, b.span.start, b.span.end, attrs, lines
            );
        }
    }
}

fn write_html_docs(
    path: &str,
    out_dir: &str,
    exports: &[String],
    fns: &[String],
    blocks: Vec<pblang_core::PolyglotBlock>,
) {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>pblang module docs</title>
  <style>
body { font-family: system-ui, sans-serif; max-width: 800px; margin: 1rem auto; padding: 0 1rem; }
h1 { font-size: 1.5rem; }
h2 { font-size: 1.2rem; margin-top: 1.5rem; }
code { background: #f0f0f0; padding: 0.1em 0.3em; border-radius: 3px; }
ul { list-style: none; padding-left: 0; }
li { margin: 0.3em 0; }
  </style>
</head>
<body>
  <h1>Module docs</h1>
  <p><strong>file</strong>: <code>"#,
    );
    html.push_str(path);
    html.push_str(
        r#"</code></p>
  <h2>Exports</h2>
  <ul>
"#,
    );
    if exports.is_empty() {
        html.push_str("    <li>(none)</li>\n");
    } else {
        for e in exports {
            html.push_str("    <li><code>");
            html.push_str(&e.replace('<', "&lt;").replace('>', "&gt;"));
            html.push_str("</code></li>\n");
        }
    }
    html.push_str("  </ul>\n  <h2>Top-level functions</h2>\n  <ul>\n");
    if fns.is_empty() {
        html.push_str("    <li>(none)</li>\n");
    } else {
        for f in fns {
            html.push_str("    <li><code>");
            html.push_str(&f.replace('<', "&lt;").replace('>', "&gt;"));
            html.push_str("</code></li>\n");
        }
    }
    html.push_str("  </ul>\n  <h2>Polyglot blocks</h2>\n  <ul>\n");
    if blocks.is_empty() {
        html.push_str("    <li>(none)</li>\n");
    } else {
        for b in blocks {
            let name = b.name.as_deref().unwrap_or("<unnamed>");
            let block_args = b.args.as_deref().unwrap_or("");
            let attrs = render_polyglot_attrs(&b.attrs);
            let lines = polyglot_code_line_count(&b.code);
            html.push_str(&format!(
                "    <li><code>@{} {} {}</code> (span {}..{}, attrs: {}, code lines: {})</li>\n",
                b.lang, name, block_args, b.span.start, b.span.end, attrs, lines
            ));
        }
    }
    html.push_str("  </ul>\n</body>\n</html>\n");

    let out_path = std::path::Path::new(out_dir);
    let _ = std::fs::create_dir_all(out_path);
    let index = out_path.join("index.html");
    if let Err(e) = std::fs::write(&index, html) {
        eprintln!(
            "pblang --doc --html: failed to write {}: {}",
            index.display(),
            e
        );
        std::process::exit(1);
    }
    println!("wrote {}", index.display());
}
