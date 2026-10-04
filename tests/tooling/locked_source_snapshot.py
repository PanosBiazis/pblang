"""Independent tree-v2/native-lock compatibility using disposable projects only."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import time
import tomllib


def tree_digest(root, legacy=False):
    digest = hashlib.sha256(b'' if legacy else b'pblang-package-tree-v2\0')
    for path in sorted(root.rglob('*'), key=lambda p: p.relative_to(root).as_posix().encode()):
        name = path.relative_to(root).as_posix().encode()
        data = path.read_bytes() if path.is_file() else b''
        if legacy:
            if path.is_file():
                digest.update(name + b'\0' + data + b'\0')
        else:
            digest.update(b'f' if path.is_file() else b'd')
            digest.update(len(name).to_bytes(8, 'big'))
            digest.update(name)
            digest.update(len(data).to_bytes(8, 'big'))
            digest.update(data)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runtime', required=True)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--report', required=True)
    parser.add_argument('--baseline', help='Old snapshot module; record the known failure only')
    opts = parser.parse_args()
    binary = Path(opts.runtime).resolve()
    repo = Path(opts.repo).resolve()
    report = Path(opts.report)
    rows = []

    def record(label, passed, **details):
        rows.append(dict(label=label, passed=bool(passed), **details))
        report.write_text(json.dumps(rows, indent=2, ensure_ascii=False) + '\n')
        print(label, bool(passed), flush=True)
        assert passed, rows[-1]

    def run(args, cwd, timeout=60):
        return subprocess.run([str(binary), *map(str, args)], cwd=cwd,
                              text=True, capture_output=True, timeout=timeout)

    with tempfile.TemporaryDirectory(prefix='pblang-locked-v2-') as temp:
        base = Path(temp)
        harness = base / 'harness'
        harness.mkdir()
        shutil.copytree(repo / 'stdlib', harness / 'stdlib')
        if opts.baseline:
            shutil.copy2(opts.baseline, harness / 'stdlib/locked_source_snapshot.sapp')
        (harness / 'pblang.toml').write_text(
            '[package]\nname="snapshot-harness"\nversion="1.0.0"\nentry="main.sapp"\n'
            '[dependencies]\nstdlib={path="' + str(harness / 'stdlib') + '"}\n')
        (harness / 'main.sapp').write_text('print "harness";')
        built = run(['build', harness], harness)
        record('harness build', built.returncode == 0, stderr=built.stderr)
        driver = harness / 'check.sapp'

        def check(root, action='snapshot'):
            if action == 'normalize':
                body = 'let L=import("stdlib:portable_lock.sapp");print json_stringify(L.publish(R));'
            elif action == 'bundle':
                body = ('let B=import("stdlib:immutable_broker_bundle.sapp");let b=B.build("main.sapp",R);'
                        'if b.ok { fs_write(R+"/linked.sapp",b.source); } print json_stringify(b);')
            else:
                body = 'let S=import("stdlib:locked_source_snapshot.sapp");print json_stringify(S.load(R));'
            driver.write_text('let R=' + json.dumps(str(root)) + ';' + body)
            started = time.monotonic()
            proc = run(['run', driver, '--allow-all'], harness)
            lines = [line for line in proc.stdout.splitlines() if line.startswith('{')]
            result = json.loads(lines[-1]) if lines else {}
            return proc, result, round(time.monotonic() - started, 3)

        def fixture(label, files=None, dirs=()):
            root = base / label
            root.mkdir()
            dep = root / 'dep'
            dep.mkdir()
            for directory in dirs:
                (dep / directory).mkdir(parents=True, exist_ok=True)
            contents = {'value.sapp': b'fn value() { return 42; }\nexport value;\n'}
            contents.update(files or {})
            for name, data in contents.items():
                (dep / name).parent.mkdir(parents=True, exist_ok=True)
                (dep / name).write_bytes(data)
            (root / 'main.sapp').write_text('let D=import("dep:value.sapp");print D.value();\n')
            (root / 'pblang.toml').write_text(
                '[package]\nname="fixture"\nversion="1.0.0"\nentry="main.sapp"\n'
                '[dependencies]\ndep={path="dep"}\n')
            before = {p.relative_to(dep).as_posix(): p.read_bytes() if p.is_file() else None
                      for p in dep.rglob('*')}
            built = run(['build', root], root)
            assert built.returncode == 0, built.stderr
            lock = tomllib.loads((root / 'pblang.lock').read_text())
            oracle = tree_digest(dep)
            record(label + ' native digest equals oracle', lock['dependencies']['dep']['sha256'] == oracle)
            proc, result, _ = check(root, 'normalize')
            record(label + ' portable lock', proc.returncode == 0 and result.get('ok'), stderr=proc.stderr)
            return root, oracle, before

        def expect(root, label, ok, digest=None, backend=None):
            proc, result, seconds = check(root)
            passed = proc.returncode == 0 and result.get('ok') == ok
            if ok:
                passed = passed and result.get('dependency_digests', {}).get('dep') == digest
                passed = passed and 'dep:value.sapp' in result.get('modules', {})
                if backend:
                    passed = passed and result.get('dependency_digest_backends', {}).get('dep') == backend
            record(label, passed, seconds=seconds, error=result.get('error'),
                   backend=result.get('dependency_digest_backends'), stderr=proc.stderr)
            return result

        root, digest, before = fixture('ascii')
        if opts.baseline:
            expect(root, 'baseline rejects genuine v2 lock', False)
            lock = root / 'pblang.lock'
            lock.write_text(lock.read_text().replace(digest, tree_digest(root / 'dep', legacy=True)))
            proc, result, _ = check(root)
            record('baseline accepts legacy bytes mislabeled v2', proc.returncode == 0 and result.get('ok'))
            return
        expect(root, 'ASCII fast path', True, digest, 'vm-sha256-utf8-canonical')
        proc, bundle, _ = check(root, 'bundle')
        record('real locked broker bundle', proc.returncode == 0 and bundle.get('ok')
               and bundle.get('lock_verified') and not bundle.get('production_execution_ready'), stderr=proc.stderr)
        linked = root / 'linked.sapp'
        # Relocate only linked text into a project with no dependencies.
        standalone = base / 'standalone'
        standalone.mkdir()
        shutil.copy2(linked, standalone / 'main.sapp')
        (standalone / 'pblang.toml').write_text('[package]\nname="linked"\nversion="1.0.0"\nentry="main.sapp"\n')
        sandbox = ['bwrap', '--unshare-all', '--die-with-parent',
                   '--ro-bind', '/usr', '/usr', '--ro-bind', '/bin', '/bin',
                   '--ro-bind', '/lib', '/lib', '--ro-bind', '/lib64', '/lib64',
                   '--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                   '--ro-bind', str(binary), str(binary),
                   '--ro-bind', str(standalone), str(standalone), '--chdir', str(standalone)]
        executed = subprocess.run(sandbox + [str(binary), 'run', str(standalone),
                                 '--deny-net', '--deny-http', '--deny-exec', '--deny-ffi'],
                                 capture_output=True, text=True, timeout=30)
        record('linked program runs without dependency project', executed.returncode == 0
               and executed.stdout.strip() == '42', stdout=executed.stdout, stderr=executed.stderr)
        lock = root / 'pblang.lock'
        valid_lock = lock.read_text()
        lock.write_text(valid_lock.replace(digest, tree_digest(root / 'dep', legacy=True)))
        expect(root, 'reject legacy hash labeled v2', False)
        lock.write_text(valid_lock)
        for name, files, dirs, backend in [
            ('empty-and-hidden', {'.hidden': b'x', 'target/data': b'y', 'κενο/α.txt': b'hello'}, ['empty', '.empty', 'looks.sapp'], 'vm-sha256-utf8-canonical'),
            ('binary', {'binary.dat': bytes(range(256))}, [], 'pure-pblang-incremental'),
            ('utf8-and-nul', {'data': 'α\0é'.encode()}, [], 'vm-sha256-utf8-canonical'),
            ('length-128', {'data': b'x' * 128}, [], 'pure-pblang-incremental'),
            ('length-256', {'data': b'x' * 256}, [], 'vm-sha256-utf8-canonical'),
            ('path-128', {'a' * 128: b'x'}, [], 'pure-pblang-incremental'),
        ]:
            case, expected, original = fixture(name, files, dirs)
            expect(case, name + ' snapshot', True, expected, backend)
            actual = {p.relative_to(case / 'dep').as_posix(): p.read_bytes() if p.is_file() else None
                      for p in (case / 'dep').rglob('*')}
            record(name + ' sources unchanged', actual == original)
            if name == 'empty-and-hidden':
                for target in ['.hidden', 'target/data', 'κενο/α.txt']:
                    p = case / 'dep' / target
                    old = p.read_bytes()
                    p.write_bytes(old + b'!')
                    expect(case, 'tamper ' + target, False)
                    p.write_bytes(old)
                (case / 'dep/empty').rmdir()
                expect(case, 'empty directory removal', False)
        (root / 'dep/value.sapp').write_bytes(b'fn value() { return 43; }\nexport value;\n')
        expect(root, 'module content tamper', False)
        (root / 'dep/value.sapp').write_bytes(before['value.sapp'])
        (root / 'dep/new-empty').mkdir()
        expect(root, 'empty directory addition', False)
        (root / 'dep/new-empty').rmdir()
        (root / 'dep/link').symlink_to(root / 'dep/value.sapp')
        expect(root, 'symlink rejected', False)
        (root / 'dep/link').unlink()
        os.mkfifo(root / 'dep/pipe')
        expect(root, 'special file rejected', False)
        (root / 'dep/pipe').unlink()
        (root / 'dep/value.sapp').write_bytes(b'\xff')
        expect(root, 'invalid UTF8 module rejected', False)
        # Restore known module; a legacy listing alias must not recurse forever.
        (root / 'dep/value.sapp').write_bytes(before['value.sapp'])
        (root / 'dep/root').mkdir()
        expect(root, 'root alias explicitly unsupported', False)


if __name__ == '__main__':
    main()
