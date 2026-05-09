//! Golden tests: run fixtures/*.sap and compare printed output to fixtures/*.expected.
//! Any change to bytecode/VM/stdio semantics should be caught by these tests.

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};
use std::fs;
use std::path::Path;
use std::rc::Rc;

fn run_fixture(sap_path: &Path) -> Result<String, String> {
    let src = fs::read_to_string(sap_path).map_err(|e| e.to_string())?;
    let stmts = parse(&src).map_err(|e| format!("parse error: {e:?}"))?;
    let mut chunk = compile(stmts);
    let name = sap_path.to_string_lossy().to_string();
    chunk.source_name = Some(Rc::new(name));
    chunk.source = Some(Rc::new(src));
    let mut vm = VM::with_stdlib(StdlibConfig::default());
    let out = vm.run_collect(chunk).map_err(|e| e.message)?;
    let lines: Vec<String> = out.iter().map(|v| v.to_string()).collect();
    Ok(lines.join("\n"))
}

#[test]
fn golden_fixtures_match_expected() {
    let fixtures_dir = Path::new("tests/fixtures");
    if !fixtures_dir.exists() {
        return;
    }
    let dir = fs::read_dir(fixtures_dir).expect("read fixtures dir");
    for ent in dir {
        let ent = ent.expect("dir entry");
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sap") {
            continue;
        }
        let expected_path = path.with_extension("expected");
        if !expected_path.exists() {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy();
        let got =
            run_fixture(&path).unwrap_or_else(|e| panic!("fixture {} run failed: {}", name, e));
        let expected = fs::read_to_string(&expected_path).expect("read expected");
        let expected = expected.replace("\r\n", "\n").trim_end().to_string();
        let got = got.trim_end();
        assert_eq!(
            got, expected,
            "fixture {} output mismatch\n--- expected\n{}\n--- got\n{}",
            name, expected, got
        );
    }
}
