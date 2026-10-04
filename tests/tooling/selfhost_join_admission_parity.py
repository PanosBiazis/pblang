"""Compare retained join primitive against all six self-host execution routes."""
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
    parser.add_argument('--count', type=int, default=9)
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
        ('join non-array', 'str_join(nil,"x");', False),
        ('join first bad argument', 'str_join(map{},nil);', False),
        ('join boxed array', 'str_join(bigint("2"),"x");', False),
        ('join separator', 'str_join([],nil);', False),
        ('join boxed separator', 'str_join(["a"],decimal("1"));', False),
        ('join generic operator', 'let receiver=map{__add__:str_join};receiver+"x";', False),
        ('join Unicode and empty', 'print str_join([],"x");print str_join([""],"");print str_join(["α🙂","á"],"λβ");', True),
        ('join non-string privacy', 'print str_join(["α",nil,7,bigint("3"),decimal("2"),rational(1,2),["synthetic"],map{data:"hidden"},false,"🙂"],"|");', True),
        ('join caught and shadowed', 'print (try str_join(nil,"x") catch false);fn str_join(a,b){return nil;}print str_join(nil,nil);', True),
    ]
    with tempfile.TemporaryDirectory(prefix='.join-admission-', dir=repo) as directory:
        root = Path(directory)
        native = root / 'native.sapp'
        driver = root / 'driver.sapp'
        for name, source, valid in cases[args.start:args.start + args.count]:
            native.write_text(source)
            expected = run(native)
            assert (expected['exit'] == 0) == valid, (name, expected)
            for mode in ['plain', 'optimized', 'pbcs']:
                for bounded in [False, True]:
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
        # Force a secondary ownership-close failure, then verify the first host
        # diagnostic is retained, not replaced by cleanup arithmetic's error.
        source = 'str_join(nil,"x");'
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
