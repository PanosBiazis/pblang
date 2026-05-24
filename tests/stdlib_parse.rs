//! Parse and typecheck stdlib files (no execution). Catches regressions in parser/typechecker and stdlib syntax.

use pblang_core::parse;
use pblang_typeck::check as type_check;
use std::fs;
use std::path::Path;

fn parse_and_typecheck(path: &Path) -> Result<(), String> {
    let src = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let stmts = parse(&src).map_err(|e| format!("parse error: {e:?}"))?;
    let diags = type_check(&stmts);
    if !diags.is_empty() {
        return Err(diags
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join("; "));
    }
    Ok(())
}

#[test]
fn stdlib_graphics_parses_and_typechecks() {
    let path = Path::new("stdlib/graphics.sapp");
    if !path.exists() {
        return;
    }
    if let Err(e) = parse_and_typecheck(path) {
        panic!("stdlib/graphics.sapp should parse and typecheck: {}", e);
    }
}
