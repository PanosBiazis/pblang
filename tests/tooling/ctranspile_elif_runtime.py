#!/usr/bin/env python3
"""Execute the functions-only C lowering against the same PBLang fixture.

Run from the repository root; --pblang selects the reviewed retained host.
No device I/O, network, firmware upload, or production installation occurs.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

SOURCE = """
fn choose(x) {
    if x == 0 { return 10; }
    elif x == 1 { return 21; }
    elif x == 2 { return 32; }
    else { return 43; }
}
fn nested(x) {
    if x == 0 { return 5; }
    elif x > 0 {
        if x == 1 { return 6; }
        elif x == 2 { return 7; }
        else { return 8; }
    }
    return 9;
}
fn count() {
    let i = 0; let total = 0;
    while i < 8 {
        i = i + 1;
        if i == 1 { total = total + 10; }
        elif i == 2 { continue; }
        elif i == 4 { break; }
        else { total = total + i; }
    }
    return total;
}
"""
CALLS = ["choose(0)", "choose(1)", "choose(2)", "choose(3)",
         "nested(0)", "nested(1)", "nested(2)", "nested(3)", "nested(-1)", "count()"]
EXPECTED = "10\n21\n32\n43\n5\n6\n7\n8\n9\n13\n"


def checked(argv):
    result = subprocess.run(argv, capture_output=True, text=True, timeout=30, check=False)
    if result.returncode:
        raise RuntimeError(f"subprocess failed ({result.returncode}): {result.stderr}")
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pblang", required=True)
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    pb = str(Path(args.pblang).resolve(strict=True))
    cc = shutil.which(args.cc)
    if cc is None:
        raise RuntimeError("C compiler required; native checks cannot be skipped")
    flags = ["--deny-net", "--deny-http", "--deny-exec", "--deny-spawn", "--deny-gfx", "--deny-sqlite"]
    with tempfile.TemporaryDirectory(prefix="pblang-elif-") as name:
        root = Path(name)
        vm = root / "fixture.sapp"
        vm.write_text(SOURCE + "\n".join("print " + call + ";" for call in CALLS), encoding="utf-8")
        reference = checked([pb, "run", str(vm), *flags])
        if reference != EXPECTED:
            raise AssertionError("PBLang reference differs from the fixture contract")
        output = root / "fixture.c"
        wrapper = root / "lower.sapp"
        wrapper.write_text('let C = import("stdlib/compiler/ctranspile.sapp");\n'
                           + "fs_write(" + json.dumps(str(output)) + ", C.transpile("
                           + json.dumps(SOURCE) + "));\n", encoding="utf-8")
        checked([pb, "run", str(wrapper), *flags])
        with output.open("a", encoding="utf-8") as stream:
            stream.write('\n#include <stdio.h>\nint main(void) {\n')
            for call in CALLS:
                stream.write('printf("%.17g\\n", ' + call + ');\n')
            stream.write("return 0;\n}\n")
        for optimization in ["-O0", "-O2"]:
            binary = root / ("native" + optimization)
            checked([cc, "-std=c99", optimization, "-Werror=return-type", str(output), "-o", str(binary)])
            if checked([str(binary)]) != reference:
                raise AssertionError("functions-only C output differs from PBLang: " + optimization)
    print("ctranspile_elif_runtime_pass: 10 values x 2 C optimization levels; temporary outputs removed")


if __name__ == "__main__":
    main()
