"""Qualify opt-in bounded admission through a prepared Linux launcher."""
import argparse
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import threading


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--group', choices=['source', 'argv', 'permissions', 'allocation', 'library'], required=True)
    a = ap.parse_args()
    release = a.candidate.resolve()
    rows = []
    # Exact libc/symbol grants, never --allow-all. Keep the source checkout hidden.
    libc = next(Path(p) for p in ['/usr/lib/x86_64-linux-gnu/libc.so.6',
                                '/lib/x86_64-linux-gnu/libc.so.6',
                                '/usr/lib/aarch64-linux-gnu/libc.so.6'] if Path(p).is_file())
    grants = ['--ffi-lib='+str(libc)] + ['--ffi-symbol='+s for s in
              ['free', 'close', 'dup', 'strdup', 'openat', 'memfd_create', 'sendfile', 'lseek']]
    with tempfile.TemporaryDirectory(prefix='pblang-bounded-source-') as tmp:
        root = Path(tmp)
        valid = 'print "γειά";'.encode()
        (root/'valid.sapp').write_bytes(valid)
        (root/'empty.sapp').write_bytes(b'')
        (root/'invalid.sapp').write_bytes(b'print "\xff";')
        (root/'type.sapp').write_text('let n:number="wrong";')
        (root/'root').write_bytes(valid)
        (root/'folder').mkdir()
        (root/'link.sapp').symlink_to('valid.sapp')
        (root/'parent-link').symlink_to('folder', target_is_directory=True)
        (root/'folder/valid.sapp').write_bytes(valid)
        os.mkfifo(root/'fifo')
        sock = socket.socket(socket.AF_UNIX)
        sock.bind(str(root/'socket'))
        box = ['bwrap', '--unshare-all', '--die-with-parent']
        for d in ['/usr', '/bin', '/lib', '/lib64']:
            box += ['--ro-bind', d, d]
        box += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                '--ro-bind', str(release), str(release), '--bind', str(root), str(root),
                '--chdir', str(root)]
        common = ['--check', '--via-pblang']

        def check(name, args, code, contains='', trace=False, env=None):
            prefix = []
            if trace:
                prefix = ['strace', '-f', '-yy', '-e', 'trace=openat,read,sendfile,memfd_create,lseek',
                          '-o', str(root/'trace.log')]
            p = subprocess.run(prefix + box + [str(release/'bin/pblang'), *args],
                               text=True, capture_output=True, timeout=10,
                               env=os.environ | (env or {}))
            ok = (p.returncode == code if code is not None else p.returncode != 0)
            ok = ok and contains in p.stdout + p.stderr
            row = dict(name=name, passed=ok, exit=p.returncode, stdout=p.stdout, stderr=p.stderr)
            rows.append(row)
            a.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False))
            print(name, ok, flush=True)
            assert ok, row
            return row

        def bounded(name, path='valid.sapp', limit=64, code=0, flags=None, contains=''):
            return check(name, common + [f'--bounded-source={limit}', *grants,
                         *(flags or []), '--', str(path)], code, contains)

        if a.group == 'source':
            bounded('exact UTF-8 byte boundary', limit=len(valid), contains='self-hosted check ok')
            bounded('one byte too small', limit=len(valid)-1, code=1, contains='exceeds byte limit')
            bounded('empty', 'empty.sapp', code=1, contains='self-hosted check failed')
            bounded('invalid UTF-8', 'invalid.sapp', code=1, contains='not valid UTF-8')
            bounded('type diagnostic', 'type.sapp', code=1, contains='self-hosted check failed')
            bounded('literal root', 'root')
            for path in ['absent', 'folder', 'link.sapp', 'parent-link/valid.sapp', 'fifo', 'socket']:
                bounded(path, path, code=1, contains='bounded source rejected')
            for path in ['χώρος πηγής.sapp', 'line\nnext.sapp', '--bounded-source=1', '--ffi-symbol=close', '--ffi-lib=not-a-library']:
                (root/path).write_bytes(valid)
                bounded('literal '+repr(path), path)
            bounded('absolute path', root/'valid.sapp')
            (root/'effect.sapp').write_text('fs_write("effect", "must-not-run");')
            bounded('source never executed', 'effect.sapp')
            assert not (root/'effect').exists()
        elif a.group == 'argv':
            for value in ['', '0', '-1', '+1', '1.5', '1e3', '1048577', '999999999999999999', '１２', ' 20']:
                check('invalid limit '+repr(value), common+[f'--bounded-source={value}', 'absent'], 2)
            for selectors in [[], ['--check'], ['--via-pblang']]:
                check('bounded requires selectors '+repr(selectors), selectors+['--bounded-source=64','valid.sapp'], 2)
            check('bounded option before selectors', ['--bounded-source=64',*common,'valid.sapp'], 2)
            check('duplicate option', common+['--bounded-source=32', '--bounded-source=64', 'absent'], 2)
            check('missing value form', common+['--bounded-source', '32', 'absent'], 2)
            check('option after filename', common+['valid.sapp', *grants, '--bounded-source=64'], 0)
            check('alias and leading capabilities', ['--allow-ffi','--self-host','--bounded-source=64','--check',*grants,'valid.sapp'], 0)
            check('grants require opt-in', common+[*grants,'valid.sapp'], 2)
            check('malformed symbol', common+['--bounded-source=64','--ffi-symbol=bad-symbol','valid.sapp'], 2)
            check('empty library grant', common+['--bounded-source=64','--ffi-lib=','valid.sapp'], None)
            bounded('upper bound', limit=1048576)
        elif a.group == 'permissions':
            # Explicit grants must not override denies or the locked sandbox.
            for deny in ['ffi', 'fs', 'import']:
                bounded('deny '+deny, flags=['--deny-'+deny], code=None)
            for deny in ['exec', 'net', 'http', 'env', 'spawn', 'gfx', 'sqlite']:
                bounded('independent deny '+deny, flags=['--deny-'+deny])
            bounded('locked sandbox', flags=['--sandbox','--allow-all'], code=None)
            check('default has no FFI grant', common+['--bounded-source=64','valid.sapp'], 1,
                  'bounded source rejected', env={'PBLANG_SANDBOX':'0'})
            check('allow-ffi alone is insufficient', common+['--bounded-source=64','--allow-ffi','valid.sapp'], 1,
                  'bounded source rejected')
            check('missing cleanup symbol', common+['--bounded-source=64',*[g for g in grants if g != '--ffi-symbol=free'],'valid.sapp'], 1,
                  'bounded source rejected')
            check('missing copy symbol', common+['--bounded-source=64',*[g for g in grants if g != '--ffi-symbol=sendfile'],'valid.sapp'], 1,
                  'bounded source rejected')
            check('ordinary route still permits denied FFI and exec', common+['--deny-ffi','--deny-exec','valid.sapp'], 0)
        elif a.group == 'library':
            module = json.dumps(str(release/'stdlib/compiler/bounded_source.sapp'))
            driver = root/'library.sapp'
            driver.write_text('let R=import(' + module + ');' + r'''
for limit in [0, -1, 1048577, 1.5, "32", nil, true] {
    if R.read_source("valid.sapp", limit).ok { throw "invalid limit accepted"; }
}
for path in [nil, 0, true, "", "folder/", "../valid.sapp"] {
    if R.read_source(path, 64).ok { throw "invalid path accepted"; }
}
let loaded=R.read_source("valid.sapp");
if not loaded.ok or loaded.size != 17 { throw "default read failed"; }
fs_write("valid.sapp", "let n:number=\"changed\";");
if loaded.source != "print \"γειά\";" { throw "captured source changed"; }
let changed=R.read_source("valid.sapp", 64);
if not changed.ok or changed.source == loaded.source { throw "replacement not observed"; }
print "library contract ok";
''')
            check('library limits, invalid paths, default, captured bytes and reread',
                  ['run', str(driver), *grants], 0, 'library contract ok')
        else:
            # A sparse 4 GiB source cannot be materialized by this test. Trace the
            # actual source descriptor and cap+1 copy, rather than trusting RSS.
            with (root/'huge.sapp').open('wb') as f:
                f.truncate(4*1024**3)
            row = check('4 GiB sparse source bounded before VM allocation',
                        common+['--bounded-source=4096',*grants,'huge.sapp'], 1,
                        'exceeds byte limit', trace=True)
            trace = (root/'trace.log').read_text()
            copies = [line for line in trace.splitlines() if 'sendfile(' in line and 'huge.sapp' in line]
            assert len(copies) == 1 and re.search(r', 4097\)\s+= 4097$', copies[0]), copies
            assert not any(re.search(r'\bread\(', line) and 'huge.sapp' in line for line in trace.splitlines())
            assert not any(re.search(r'\bread\(', line) and 'memfd:pblang-bounded-read' in line for line in trace.splitlines())
            row['source_copy_bytes'] = 4097
            row['source_read_calls'] = 0
            a.report.with_suffix('.trace.log').write_text(trace)
            # Concurrent growth does not bypass the limit. This is not a proof
            # of an atomic point-in-time view during arbitrary in-place writes.
            growing = root/'growing.sapp'
            growing.write_bytes(b' '*8192)
            stopped = threading.Event()
            def grow():
                with growing.open('ab', buffering=0) as f:
                    while not stopped.wait(.002):
                        if f.tell() < 8*1024**2:
                            f.write(b' '*8192)
            writer = threading.Thread(target=grow)
            writer.start()
            try:
                bounded('concurrently growing source', 'growing.sapp', limit=4096, code=1, contains='exceeds byte limit')
            finally:
                stopped.set(); writer.join()
            assert growing.stat().st_size > 8192
            # Replacing the path between checks must use fresh source content.
            bounded('before replacement')
            (root/'replacement').write_text('let n:number="bad";')
            os.replace(root/'replacement', root/'valid.sapp')
            bounded('after replacement', code=1, contains='self-hosted check failed')
        sock.close()
        a.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False))


if __name__ == '__main__':
    main()
