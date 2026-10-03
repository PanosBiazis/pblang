"""Real installed compiler/checker journeys with no source checkout visible (Linux)."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--report', type=Path)
    options = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    binary = (options.binary or repo / 'target/release/pblang').resolve()
    records = []

    def run(label, argv, cwd, expected=0, output=None):
        started = time.monotonic()
        result = subprocess.run(list(map(str, argv)), cwd=cwd, capture_output=True,
                                text=True, timeout=45)
        ok = (result.returncode == expected if expected is not None else result.returncode != 0)
        ok = ok and (output is None or result.stdout.strip() == output)
        ok = ok and 'DECOY_EXECUTED' not in result.stdout + result.stderr
        ok = ok and 'SOURCE_EXECUTED' not in result.stdout + result.stderr
        records.append(dict(case=label, passed=ok, exit=result.returncode,
                            seconds=round(time.monotonic() - started, 3),
                            stdout=result.stdout[-1500:], stderr=result.stderr[-1500:]))
        if options.report:
            options.report.write_text(json.dumps(records, indent=2) + '\n')
        assert ok, records[-1]
        return result

    with tempfile.TemporaryDirectory(prefix='pbc-portability-') as folder:
        root = Path(folder)
        installed = root / 'installed'
        installed.mkdir()
        (installed / 'bin').mkdir()
        exe = installed / 'bin/pblang'
        shutil.copy2(binary, exe)
        shutil.copytree(repo / 'stdlib', installed / 'stdlib')
        (installed / 'tools').mkdir()
        for name in ['pbc.sapp', 'pbcheck.sapp', 'pblock_install.sapp']:
            shutil.copy2(repo / 'tools' / name, installed / 'tools' / name)
        manifest = '[package]\nname="portability"\nversion="0.1.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="stdlib"}\n'
        (installed / 'pblang.toml').write_text(manifest)
        (installed / 'main.sapp').write_text('print 0;')
        (installed / 'guarded-library.sapp').write_text(
            'let B=import("stdlib:compiler/native_bundle.sapp");'
            'let source="fn init(){print 1;}fn work(){print 2;}fn cleanup(){print 3;}";'
            'fs_write_bytes("direct.pbck",B.compile_bundle_with_lifecycle(source,fn(p){return nil;},"init","work","cleanup"));')
        run('installed build', [exe, 'build'], installed)
        run('installed absolute lock', [exe, 'run', 'tools/pblock_install.sapp', '--allow-all', '--', '.'], installed)
        for mode in ['manifestless', 'locked-decoy']:
            caller = root / mode
            caller.mkdir()
            if mode == 'locked-decoy':
                (caller / 'stdlib/compiler').mkdir(parents=True)
                (caller / 'pblang.toml').write_text(manifest)
                (caller / 'main.sapp').write_text('print 0;')
                for name in ['compiler/native_pbck.sapp', 'compiler/native_bundle.sapp',
                             'compiler/source_analysis.sapp', 'controlled_modularity_file.sapp',
                             'codegen_wasm.sapp', 'codegen_arm64_ast.sapp']:
                    (caller / 'stdlib' / name).write_text('print "DECOY_EXECUTED";')
                run('decoy build', [exe, 'build'], caller)
            # A minimal root exposes only the installed payload, fixtures and OS tools.
            sandbox = ['bwrap', '--unshare-all', '--die-with-parent', '--ro-bind', '/usr', '/usr',
                       '--ro-bind', '/bin', '/bin', '--ro-bind', '/lib', '/lib', '--ro-bind', '/lib64', '/lib64',
                       '--ro-bind', '/etc/alternatives', '/etc/alternatives',
                       '--ro-bind', '/etc/ld.so.cache', '/etc/ld.so.cache', '--proc', '/proc', '--dev', '/dev',
                       '--tmpfs', '/tmp', '--ro-bind', installed, installed, '--bind', caller, caller,
                       '--chdir', caller, '--unsetenv', 'PBLANG_PATH']
            run(mode + ' source unavailable', sandbox + ['/bin/sh', '-c', 'test ! -e "$1"', 'probe', repo], caller)

            def child(label, args, **kw):
                return run(mode + ' ' + label, sandbox + [exe] + args, caller, **kw)

            def compile_(name, source, flags=(), expected=0):
                (caller / name).write_text(source)
                return child('compile ' + name, ['run', installed / 'tools/pbc.sapp', '--allow-all', '--', name, *flags], expected=expected)

            child('direct guarded library', ['run', installed / 'guarded-library.sapp', '--allow-all'])
            child('direct guarded library artifact', ['run', 'direct.pbck', '--sandbox'], output='1\n2\n3')
            compile_('defaults with space.sapp', 'fn f(x=7){return x;}print [f(),f(nil),f(false)];',
                     ['--target', 'pbck'])
            child('default output and explicit nil', ['run', 'defaults with space.pbck', '--sandbox'], output='[7, nil, false]')
            child('compile without FFI exec network', ['run', installed / 'tools/pbc.sapp', '--allow-all',
                  '--deny-ffi', '--deny-exec', '--deny-net', '--deny-http', '--deny-spawn', '--',
                  'defaults with space.sapp', '--target', 'pbck', '--out', 'limited.pbck'])
            child('limited compiler artifact', ['run', 'limited.pbck', '--sandbox'], output='[7, nil, false]')
            (caller / 'dep.sapp').write_text('let value=31;export value;')
            program = 'let d=import("dep.sapp");fn init(){print "INIT";}fn work(){print d.value;}fn cleanup(){print "CLEAN";}'
            for guarded in ['plain', 'cleanup', 'lifecycle']:
                source = program + ('work();' if guarded == 'plain' else '')
                flags = ['--target', 'pbck', '--bundle', '--out', guarded + '.pbck']
                if guarded != 'plain':
                    flags += ['--entry', 'work', '--cleanup', 'cleanup']
                if guarded == 'lifecycle':
                    flags += ['--initialize', 'init']
                compile_(guarded + '.sapp', source, flags)
                child('execute ' + guarded, ['run', guarded + '.pbck', '--sandbox'],
                      output={'plain': '31', 'cleanup': '31\nCLEAN', 'lifecycle': 'INIT\n31\nCLEAN'}[guarded])
            compile_('single-lifecycle.sapp', 'fn init(){print "INIT";}fn work(){print 7;}fn cleanup(){print "CLEAN";}',
                     ['--target', 'pbck', '--out', 'single.pbck', '--initialize', 'init', '--entry', 'work', '--cleanup', 'cleanup'])
            child('execute single lifecycle', ['run', 'single.pbck', '--sandbox'], output='INIT\n7\nCLEAN')
            (caller / 'contract.json').write_text(json.dumps(dict(format='pblang.controlled-modularity.v1', modules=[
                dict(id='app', version='1.0.0', interfaces={}, capabilities=[], dependencies=[])])))
            compile_('contract.sapp', 'print 7;', ['--target', 'pbck', '--module-contract', 'contract.json', '--module-root', 'app'])
            child('execute admitted contract', ['run', 'contract.pbck', '--sandbox'], output='7')
            for target in ['x64', 'arm64', 'wasm']:
                compile_(target + '.sapp', 'print 7;', ['--target', target, '--out', target + '.bin'])
                data = (caller / (target + '.bin')).read_bytes()
                assert data.startswith(b'\x00asm' if target == 'wasm' else b'\x7fELF')
                if target == 'arm64':
                    assert int.from_bytes(data[18:20], 'little') == 183
                if target == 'x64':
                    (caller / 'x64.bin').chmod(0o700)
                    run(mode + ' native x64 execution', sandbox + [caller / 'x64.bin'], caller, output='7')
                if target == 'wasm':
                    run(mode + ' wasm instantiation', sandbox + ['node', '-e',
                        "WebAssembly.instantiate(require('fs').readFileSync('wasm.bin'),{env:{print:v=>console.log(String(v)),print_str:()=>{throw new Error('unexpected string print')}}}).then(r=>{if(typeof r.instance.exports.main!=='function')process.exit(1);r.instance.exports.main();console.log('wasm_pass')})"], caller, output='7\nwasm_pass')
            (caller / 'preserved.pbck').write_text('unchanged')
            for source, flags in [('print missing_name;', []), ('print 7;', ['--safe']),
                                  ('print 7;', ['--module-contract', 'contract.json', '--module-root', 'missing'])]:
                compile_('rejected.sapp', source, ['--target', 'pbck', '--out', 'preserved.pbck', *flags], expected=None)
                assert (caller / 'preserved.pbck').read_text() == 'unchanged'
            compile_('capability.sapp', 'fs_read("NOT_READ");', ['--target', 'pbck'])
            denied = child('artifact retains denied fs', ['run', 'capability.pbck', '--sandbox'], expected=None)
            assert 'fs access denied' in denied.stderr
            for name, source, code in [('good.sapp', 'print "SOURCE_EXECUTED";', 0),
                                       ('bad.sapp', 'print missing_name;', None), ('broken.sapp', 'let x = ;', None)]:
                (caller / name).write_text(source)
                child('public check ' + name, ['check', name], expected=code)
                child('PBLang checker ' + name, ['run', installed / 'tools/pbcheck.sapp', '--', name], expected=code)
        # Known legacy behavior is recorded separately, not accepted as a useful checker.
        (installed / 'bad.sapp').write_text('print missing_name;')
        legacy = run('legacy via-pblang failure prints success exit', [exe, '--check', '--via-pblang', 'bad.sapp'], installed)
        assert 'self-hosted check failed' in legacy.stdout
    print(json.dumps(dict(passed=len(records), temporary_fixture_removed=True)))


if __name__ == '__main__':
    main()
