# pblang → WebAssembly

pblang compiles straight to **WebAssembly** — the binary format every browser, node, deno, wasmtime and
wasmer executes natively. This is pblang's web / universal-runtime target: the same `.sapp` source that
becomes a native x86-64 or ARM64 ELF becomes a `.wasm` module with one flag.

```sh
pblang run tools/pbc.sapp your_program.sapp --target wasm --out your_program.wasm
```

`fib.wasm` here (86 bytes!) was built exactly this way from `fib.sapp` and computes `fib(30)`:

```sh
node -e "WebAssembly.instantiate(require('fs').readFileSync('demo/wasm/fib.wasm')).then(r=>console.log(String(r.instance.exports.main())))"
# → 832040
```

In a browser it's the same three lines of loader glue — the *program logic* is 100% pblang:

```html
<script>
fetch('fib.wasm').then(r => r.arrayBuffer())
  .then(b => WebAssembly.instantiate(b))
  .then(r => console.log(String(r.instance.exports.main())));
</script>
```

## The browser demo (`index.html` + `show.sapp` → `show.wasm`)

```sh
cd demo/wasm && python3 -m http.server     # then open http://localhost:8000/
```

`show.wasm` (397 bytes, compiled from `show.sapp`) prints a greeting, the squares of 1–10, the first 12
Fibonacci numbers, and `gcd(1071, 462)` — **all program logic is pblang**; the page contains only a
12-line loader that wires `print`/`print_str` to the DOM. Verified live in Chrome.

## What the backend supports (steps 1–3)

Numbers (i64), `let`/assign, `+ - * / %` with precedence, unary minus, comparisons (as 0/1 values),
short-circuit `and`/`or`, `if`/`elif`/`else`, `while`, functions + parameters + `return`, calls,
recursion (incl. mutual), `print`, and **heap strings**: string values are `[len][bytes]` objects in
linear memory managed by a bump-pointer global (the wasm twin of the ARM64 backend's `x28` arena) —
**concatenation, `str(n)`, `len(s)`, `==`/`!=`, string parameters and string-returning functions** all
work, implemented by four wasm helper functions (`__concat`/`__str`/`__streq`/`__prints`) the compiler
appends to the module. Interprocedural type inference (shared design with the ARM64 backend) types string
params/returns statically. Modules that never print or touch strings import nothing. Entry convention
matches the x64/ARM64 backends. The module exports `main` returning i64 (+ `memory` when strings exist).

## First-class CLI

The Rust executable now routes the pure-pblang cross backends directly:

```sh
pblang compile prog.sapp --target wasm  --out prog.wasm    # WebAssembly (this folder)
pblang compile prog.sapp --target arm64 --out prog.elf     # static AArch64 ELF
pblang compile prog.sapp                                    # native via C99/cranelift/LLVM as before
```

Host glue (node or browser — same 3 lines):

```js
const env = { print: v => out(String(v)),
              print_str: (p, l) => out(new TextDecoder().decode(new Uint8Array(mem.buffer, p, l))) };
WebAssembly.instantiate(bytes, { env }).then(r => { mem = r.instance.exports.memory; r.instance.exports.main(); });
```

## How it's verified

`tests/semantics/codegen_wasm_test.sapp` compiles 32 programs and executes every emitted module in a
**real production engine** (node's V8), checking `main()`'s result and the full printed output —
arithmetic, control flow, recursion (`fib`, `fact`, `gcd`, mutual `even`/`odd`), short-circuit semantics
(proved by an avoided div-by-zero trap), number/string/mixed prints, and the entry convention. The engine
also *validates* each module byte-for-byte, so the binary format itself is externally certified. The
browser page was additionally loaded in Chrome and its rendered output checked line-by-line.
