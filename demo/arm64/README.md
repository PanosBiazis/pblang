# pblang → ARM64 native binaries

## Compile your own (the `--target arm64` flag)

From an x86-64 machine, cross-compile any pblang program to a static ARM64 ELF — no cross-toolchain:

```sh
pblang run tools/pbc.sapp your_program.sapp --target arm64 --out your_program.elf
# then copy your_program.elf to an arm64 device and run it
```

`--target` defaults to `x64` (native x86-64). `fibseq.elf` here was built exactly this way from `fibseq.sapp`
(prints the first 10 Fibonacci numbers). Entry convention matches the x64 backend: top-level statements are the
program body; a bare `fn main()` runs only when there's no top-level code.

---


These are **real, statically-linked AArch64 (ARM64) Linux executables**, compiled from pblang source by the
pblang-written backend (`stdlib/codegen_arm64_ast.sapp`). No libc, no runtime, no dynamic linker — the kernel
loads the single RWX segment and jumps to the entry point. Each program exits with its computed result, so you
verify it with `echo $?`.

| binary | pblang source | expected `echo $?` |
|---|---|---|
| `fact5.elf`   | `fn fact(n){ if n<2 {return 1;} return n*fact(n-1); } fn main(){ return fact(5); }` | **120** |
| `fib10.elf`   | `fn fib(n){ if n<2 {return n;} return fib(n-1)+fib(n-2); } fn main(){ return fib(10); }` | **55** |
| `loopsum.elf` | `fn main(){ let i=0; let s=0; while i<10 { s=s+i; i=i+1; } return s; }` | **45** |
| `gcd.elf`     | `fn gcd(a,b){ if b==0 {return a;} return gcd(b, a%b); } fn main(){ return gcd(48,36); }` | **12** |
| `squares.elf` | `fn main(){ let i=1; while i<=10 { print i*i; i=i+1; } return 0; }` | prints `1 4 9 … 100` (one per line), exits **0** |

| `countdown.elf` | `fn main(){ let i=5; while i >= 0-5 { print i; i=i-1; } return 0; }` | prints `5 4 3 2 1 0 -1 -2 -3 -4 -5`, exits **0** |
| `hello.elf`    | `print "Hello, ARM64!"; print "compiled by pblang";` | prints `Hello, ARM64!` / `compiled by pblang`, exits **0** |

`hello.elf` prints **string literals** — the text lives in a data section inside the ELF (you can see it with
`strings hello.elf`), and `print` writes those bytes to stdout via the `write` syscall.

| `greet.elf`   | `fn sq(n){ return n*n; } let i=1; while i<=5 { print "sq(" + str(i) + ") = " + str(sq(i)); i=i+1; }` | prints `sq(1) = 1` … `sq(5) = 25`, exits **0** |

`greet.elf` uses `str()` and string **concatenation** — these build *new* strings at runtime, so the binary
`mmap`s a heap arena at startup (into a reserved bump-pointer register) and calls two helper routines (`__str`,
`__concat`) to allocate and fill them.

| `palindrome.elf` | a `ispal(s)` checker using `s[i]`, `!=`, `len(s)`, string returns, concat | `racecar -> yes` / `hello -> no` / `level -> yes` |

`palindrome.elf` exercises the full string toolkit: **indexing** (`s[i]`), **comparison** (`!=`), **`len`**,
string-returning functions, and concatenation — a real string-processing program compiled to ARM64.

| `sort.elf`    | a bubble sort over `[5,2,8,1,9,3]` — array literal, `a[i]` read/write, `len`, nested loops | prints `1 2 3 5 8 9` |

`sort.elf` uses **arrays**: literals, indexed read/write (element swaps), `len`, and `push` (growth). Arrays are
heap objects `[count][cap][elems…]`, so array programs also `mmap` the arena at startup.

| `tally.elf`   | map counters + an array: `m.small`/`m.big` incremented over `[3,12,7,20,5,100]` | prints `small: 3` / `big: 3` |

`tally.elf` uses **maps** (string-keyed `[key,value]` pairs) alongside arrays — member get/set, `map_has`, and
growth. Map key lookup reuses the same byte-compare (`__streq`) as string equality.

| `caesar.elf`  | a Caesar cipher over `"hello world"` (`strcharcodeat` + `strfromcharcode` + concat, shift ±3) | prints `encrypted: khoor#zruog` / `decrypted: hello world` |

`caesar.elf` uses the string builtins `strcharcodeat`, `strfromcharcode`, and `str_slice` to rebuild strings
character by character — with string parameters and returns flowing through the `shift` function.

| `words.elf`   | `str_split` on a CSV line + a sentence; loops over the token arrays | `name 0: ada` … `words: 5` / `device runs` |

`words.elf` uses **`str_split`** — a string is split into an **array of heap strings** (the first
array-of-strings on this target), and the elements are string-typed: they work in `len()`, `==`, and
concatenation, not just `print`. Functions may also take **more than 8 arguments** now — args 9+ pass on
the stack per the standard AAPCS64 convention (mirrored on x86-64, where args 7+ go on the stack per SysV).

`squares.elf` and `countdown.elf` use `print` — they write to stdout via the `write` syscall, so on real hardware
you see the numbers, not just an exit code. `countdown.elf` exercises **signed/negative** print (the `-N` values).
```sh
./squares.elf     # 1 4 9 16 25 36 49 64 81 100   (one per line)
./countdown.elf   # 5 4 3 2 1 0 -1 -2 -3 -4 -5     (one per line)
```

## Run on real ARM64 hardware

Copy to any AArch64 Linux and run directly (Raspberry Pi 3/4/5, an arm64 server/VM, Apple silicon under a Linux
VM/Docker, Android via Termux, etc.):

```sh
chmod +x fact5.elf
./fact5.elf ; echo "exit = $?"     # → exit = 120
```

Quick all-in-one check:

```sh
for f in fact5 fib10 loopsum gcd; do ./$f.elf; echo "$f -> $?"; done
# fact5 -> 120 ; fib10 -> 55 ; loopsum -> 45 ; gcd -> 12
```

## How these were verified

The build host is x86-64 (no arm64 hardware / no qemu-user), so these were **not** run here. Instead:
1. `file` + `readelf -h` (binutils) confirm each is a valid statically-linked **AArch64 EXEC**, entry `0x400078`.
2. The entry decodes to the intended `bl main ; movz x8,#93 ; svc #0`.
3. Every byte of code is the exact output of `compile_ast(src)`, and that identical byte stream was executed by an
   in-pblang AArch64 interpreter (`tests/semantics/codegen_arm64_ast_test.sapp`) which checked the exit codes
   above. So on real silicon they produce the same results.

If you run them on a real device and the exit codes match the table, that's the final external confirmation.
