"""Linux source-hidden differential qualification of the private cache adapter.

Run only against prepared releases. Fixtures and all write effects are disposable;
no network namespace, developer checkout, home, or global installation is exposed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib

SYMBOLS = ['free', 'close', 'dup', 'strdup', 'openat', 'mkdirat', 'unlinkat',
           'renameat', 'fchmod', 'fsync', 'memfd_create', 'sendfile', 'lseek', 'getpid', 'getuid']


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--reference', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--group', choices=['parity', 'denials', 'integration'], required=True)
    args = ap.parse_args()
    rows = []
    source_root = Path(__file__).resolve().parents[2]
    libc = next(p for p in ['/usr/lib/x86_64-linux-gnu/libc.so.6',
                          '/lib/x86_64-linux-gnu/libc.so.6',
                          '/usr/lib/aarch64-linux-gnu/libc.so.6'] if Path(p).is_file())

    def record(label, passed, **details):
        rows.append(dict(label=label, passed=bool(passed), **details))
        args.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False) + '\n')
        print(label, bool(passed), flush=True)
        assert passed, rows[-1]

    def run(release, source, omit=None):
        release = release.resolve()
        with tempfile.TemporaryDirectory(prefix='pblang-cache-io-') as tmp:
            root = Path(tmp)
            (root/'private').mkdir(mode=0o700)
            (root/'private/folder').mkdir(mode=0o700)
            (root/'shared').mkdir(mode=0o755)
            (root/'shared').chmod(0o755)
            (root/'outside').write_text('outside')
            os.link(root/'outside', root/'private/hardlink')
            (root/'private/final-link').symlink_to('../outside')
            (root/'parent-link').symlink_to('private')
            os.mkfifo(root/'private/fifo')
            (root/'stdlib').symlink_to(release/'stdlib', target_is_directory=True)
            manifest = ('[package]\nname="cache-io-test"\nversion="1.0.0"\nentry="main.sapp"\n'
                        '[dependencies]\nstdlib={path='+json.dumps(str(release/'stdlib'))+'}\n')
            (root/'pblang.toml').write_text(manifest)
            dep = tomllib.loads((release/'pblang.lock').read_text())['dependencies']['stdlib']
            (root/'pblang.lock').write_text('manifest_sha256='+json.dumps(hashlib.sha256(manifest.encode()).hexdigest())
                +'\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies.stdlib]\npath='
                +json.dumps(str(release/'stdlib'))+'\nsha256='+json.dumps(dep['sha256'])+'\n')
            (root/'main.sapp').write_text(source)
            box = ['bwrap', '--unshare-all', '--die-with-parent', '--clearenv']
            for d in ['/usr', '/bin', '/lib', '/lib64']:
                box += ['--ro-bind', d, d]
            box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                    '--ro-bind', str(release), str(release), '--bind', tmp, tmp,
                    '--chdir', tmp, '--setenv', 'PATH', '/usr/bin:/bin', '--setenv', 'LANG', 'C.UTF-8']
            grants = ['--allow-exec', '--allow-ffi', '--ffi-lib='+libc] + ['--ffi-symbol='+s for s in SYMBOLS if s != omit]
            denies = ['--deny-'+s for s in ['net', 'http', 'env', 'spawn', 'gfx', 'sqlite']]
            if omit == 'ffi':
                denies += ['--deny-ffi']
            p = subprocess.run(box+[str(release/'bin/pblang'), 'run', 'main.sapp', *grants, *denies],
                               text=True, capture_output=True, timeout=15)
            effects = dict(outside=(root/'outside').read_text(), shared_mode=oct((root/'shared').stat().st_mode & 0o777),
                           shared_file=(root/'shared/file').exists(), stage_files=[str(p.relative_to(root)) for p in root.rglob('.pblang-*')])
            return dict(exit=p.returncode, stdout=p.stdout, stderr=p.stderr), effects

    def old_calls(source):
        source = source.replace('stdlib:compiler/module_cache_file.sapp', 'stdlib:secure_file.sapp')
        # Use tiny forwarding wrappers so nested argument expressions remain intact.
        source = source.replace('let C = import(', 'let S = import(')
        end = source.index(';', source.index('let S =')) + 1
        wrappers = ('\nfn read_bytes(p,n){return S.read_bytes(p,n,true);}'
                    '\nfn read_text(p,n){return S.read_text(p,n,true);}'
                    '\nfn write_atomic(p,b){return S.write_atomic(p,b,true,true);}'
                    '\nfn write_text_atomic(p,t){return S.write_text_atomic(p,t,true,true);}'
                    '\nfn remove(p){return S.remove(p,true);}\n')
        return source[:end] + wrappers + source[end:].replace('C.', '')

    if args.group == 'parity':
        source = (source_root/'tests/semantics/module_cache_file_test.sapp').read_text()
        expected, before = run(args.reference, old_calls(source))
        record('existing private-file contract', expected['exit'] == 0 and expected['stdout'].endswith('module_cache_file_pass\n'), result=expected)
        actual, after = run(args.candidate, source)
        record('same results and diagnostics', actual == expected, result=actual)
        record('outside inode and parent modes preserved', before == after == dict(outside='outside', shared_mode='0o755', shared_file=False, stage_files=[]))
    elif args.group == 'denials':
        source = ('let C = import("stdlib:compiler/module_cache_file.sapp");'
                  'print json_stringify(C.write_text_atomic("private/denied", "x"));'
                  'print json_stringify(C.read_bytes("private/denied", 8));')
        for omitted in ['ffi', 'free', 'close', 'strdup', 'openat']:
            expected, _ = run(args.reference, old_calls(source), omitted)
            actual, _ = run(args.candidate, source, omitted)
            record('denied '+omitted, actual == expected and actual['exit'] == 0
                   and all(json.loads(line)['ok'] is False for line in actual['stdout'].splitlines()), result=actual)
    else:
        source = (source_root/'tests/semantics/module_cache_test.sapp').read_text()
        for name, release in [('reference', args.reference), ('candidate', args.candidate)]:
            result, _ = run(release, source)
            record(name+' persistent cache integration', result['exit'] == 0 and result['stdout'] == 'module_cache_pass\n', result=result)


if __name__ == '__main__':
    main()
