"""Compare retained string primitives against all six self-host execution routes."""
import argparse
import json
import re
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', required=True)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--report', required=True)
    parser.add_argument('--lock')
    parser.add_argument('--start', type=int, default=0)
    parser.add_argument('--count', type=int, default=20)
    args = parser.parse_args()
    repo = Path(args.repo).resolve()
    report = Path(args.report)
    rows = json.loads(report.read_text()) if args.start and report.exists() else []

    def run(path):
        command = [str(Path(args.runtime).resolve()), 'run', str(path),
                   '--deny-net', '--deny-http', '--deny-exec', '--deny-ffi',
                   '--deny-spawn', '--deny-gfx', '--deny-sqlite']
        if args.lock:
            command = ['flock', '-w', '180', args.lock, *command]
        result = subprocess.run(command, cwd=repo, text=True, capture_output=True, timeout=225)
        errors = re.findall(r'^error\[.*?\]: (.*)$', result.stderr, re.M)
        return {'exit': result.returncode, 'stdout': result.stdout,
                'error': errors[0] if errors else None}

    cases = [
        ('repeat nil', 'str_repeat(nil,2);', False),
        ('repeat count type', 'str_repeat("a",nil);', False),
        ('repeat negative', 'str_repeat("a",-1);', False),
        ('repeat nan', 'str_repeat("a",num_sqrt(-1));', False),
        ('repeat boxed text', 'str_repeat(bigint("2"),2);', False),
        ('repeat boxed count', 'str_repeat("a",rational(2,1));', False),
        ('replace source', 'str_replace_all(nil,"a","b");', False),
        ('replace search', 'str_replace_all("a",nil,"b");', False),
        ('replace boxed', 'str_replace_all("a","a",decimal("1"));', False),
        ('left source', 'str_pad_left(nil,2,"b");', False),
        ('left width', 'str_pad_left("a",bigint("2"),"b");', False),
        ('left padding', 'str_pad_left("a",2,nil);', False),
        ('right source', 'str_pad_right(nil,2,"b");', False),
        ('right width', 'str_pad_right("a",nil,"b");', False),
        ('right padding', 'str_pad_right("a",2,bigint("2"));', False),
        ('first bad argument', 'str_repeat(nil,map{});', False),
        ('generic operator', 'let receiver=map{__add__:str_repeat};receiver+2;', False),
        ('accepted strings', 'print str_repeat("α",2.9);print str_repeat("a",0);print str_repeat("a",-0);'
         'print str_replace_all("aba","a","λ");print str_replace_all("a","","x");'
         'print str_pad_left("a",3.9,"λβ");print str_pad_right("a",3,"");', True),
        ('accepted nan width', 'print str_pad_left("a",num_sqrt(-1),"x");'
         'print str_pad_right("a",num_sqrt(-1),"x");', True),
        ('handled and shadowed', 'print (try str_repeat(nil,2) catch false);'
         'fn str_pad_left(a,b,c){return nil;}print str_pad_left(nil,nil,nil);', True),
    ]
    with tempfile.TemporaryDirectory(prefix='.string-admission-', dir=repo) as directory:
        root = Path(directory)
        native = root / 'native.sapp'
        driver = root / 'driver.sapp'
        for name, source, valid in cases[args.start:args.start + args.count]:
            native.write_text(source)
            expected = run(native)
            assert (expected['exit'] == 0) == valid, (name, expected)
            for mode in ['plain', 'optimized', 'pbcs']:
                for bounded in ([False] if name == 'accepted nan width' else [False, True]):
                    text = 'let F=import("stdlib:compiler/selfhost_frontend.sapp");'
                    text += 'let V=import("stdlib:compiler/vm.sapp");'
                    text += 'let O=import("stdlib:compiler/optimizer.sapp");'
                    text += 'let P=import("stdlib:selfhost_pbck.sapp");'
                    text += 'let result=F.frontend_compile(' + json.dumps(source, ensure_ascii=False) + ');'
                    text += 'assert(result.ok,"fixture compile");let code=result.chunk;'
                    if mode == 'optimized':
                        text += 'code=O.optimize_safe(code).chunk;'
                    if mode == 'pbcs':
                        text += 'code=P.deserialize_chunk(P.serialize_chunk(code));'
                    text += 'let c=V.execution_context(true);V.run_seeded(F.frontend_compile("memory_alloc(7);").chunk,map{},c);'
                    text += 'let result=V.' + ('run_seeded_bounded(code,map{},nil,c)' if bounded else 'run_seeded(code,map{},c)') + ';'
                    text += 'V.close_execution(c);for item in result.output {print item;}'
                    driver.write_text(text)
                    actual = run(driver)
                    passed = actual == expected
                    rows.append(dict(case=name, mode=mode, bounded=bounded, passed=passed,
                                     expected=expected, actual=actual))
                    report.write_text(json.dumps(rows, ensure_ascii=False, indent=2))
                    assert passed, rows[-1]
            print(name + ' PASS', flush=True)
        if args.start + args.count < len(cases):
            print("BATCH PASS " + str(len(rows)), flush=True)
            return
        # Host accepts negative width; bounded VM deliberately retains its prior
        # nonnegative-allocation policy. Compare the unbounded route separately.
        source = 'print str_pad_left("a",-1,"x");print str_pad_right("a",-num_pow(10,1000),"x");'
        native.write_text(source)
        expected = run(native)
        driver.write_text('let F=import("stdlib:compiler/selfhost_frontend.sapp");let r=F.frontend_run(' + json.dumps(source) + ');for x in r.output {print x;}')
        actual = run(driver)
        assert actual == expected and actual['exit'] == 0, (expected, actual)
        rows.append(dict(case='accepted negative width unbounded', passed=True, actual=actual))
        # Force a secondary ownership-close failure, then verify the first host
        # diagnostic is retained, not replaced by cleanup arithmetic's error.
        source = 'str_repeat(nil,2);'
        native.write_text(source)
        expected = run(native)
        driver.write_text('let F=import("stdlib:compiler/selfhost_frontend.sapp");let V=import("stdlib:compiler/vm.sapp");let c=V.execution_context(true);V.run_seeded(F.frontend_compile("memory_alloc(7);").chunk,map{},c);c.memory.reclaimed=map{};V.run_seeded(F.frontend_compile(' + json.dumps(source) + ').chunk,map{},c);')
        actual = run(driver)
        assert actual == expected and actual['exit'] != 0, (expected, actual)
        rows.append(dict(case='first error survives cleanup fault', passed=True, actual=actual))
        report.write_text(json.dumps(rows, ensure_ascii=False, indent=2))
    print('ALL PASS ' + str(len(rows)), flush=True)


if __name__ == '__main__':
    main()
