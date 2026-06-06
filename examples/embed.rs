//! Embedding example: create a VM with only the capabilities you need.
//!
//! Use `VM::new()` for zero builtins (you register your own), or
//! `VM::with_stdlib(config)` and set only the capabilities you want (e.g. fs but no network/FFI).

use pblang_compiler::compile;
use pblang_core::parse;
use pblang_vm::{StdlibConfig, VM};

fn main() {
    // Sandboxed: only fs + json, no network, no exec, no FFI
    let config = StdlibConfig {
        core: true,
        fs: true,
        net: false,
        http: false,
        json: true,
        crypto: true,
        sqlite: false,
        ffi: false,
        exec: false,
        gfx: false,
        spawn: false,
        env: false,
    };

    let mut vm = VM::with_stdlib(config);

    let script = r#"
        let data = fs_read("Cargo.toml");
        if data != nil {
            print "read ";
            print len(data);
            print " bytes";
        } else {
            print "file not found";
        }
    "#;

    let stmts = match parse(script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {e:?}");
            return;
        }
    };
    let chunk = match compile(stmts) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Compile error: {e}");
            return;
        }
    };
    if let Err(e) = vm.run(chunk) {
        eprintln!("Runtime error: {}", e.message);
    }
}
