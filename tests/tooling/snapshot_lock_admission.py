"""Focused metadata admission through the real snapshot and broker APIs."""
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import tomllib


def main():
    p = argparse.ArgumentParser()
    for name in ['runtime', 'repo', 'report']:
        p.add_argument('--' + name, required=True)
    p.add_argument('--baseline')
    opts = p.parse_args()
    binary = Path(opts.runtime).resolve()
    repo = Path(opts.repo).resolve()
    rows = []

    def record(label, passed, **data):
        rows.append(dict(label=label, passed=bool(passed), **data))
        Path(opts.report).write_text(json.dumps(rows, indent=2) + '\n')
        print(label, bool(passed), flush=True)
        assert passed, rows[-1]

    def run(args, cwd):
        return subprocess.run([str(binary), *map(str, args)], cwd=cwd,
                              capture_output=True, text=True, timeout=45)

    with tempfile.TemporaryDirectory(prefix='pblang-lock-admission-') as temp:
        base = Path(temp)
        harness = base / 'harness'
        harness.mkdir()
        shutil.copytree(repo / 'stdlib', harness / 'stdlib')
        if opts.baseline:
            shutil.copy2(opts.baseline, harness / 'stdlib/locked_source_snapshot.sapp')
        (harness / 'pblang.toml').write_text('[package]\nname="lock-harness"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\nstdlib={path="' + str(harness / 'stdlib') + '"}\n')
        (harness / 'main.sapp').write_text('print 0;')
        proc = run(['build', harness], harness)
        record('harness builds', proc.returncode == 0, stderr=proc.stderr)
        root = base / 'project'
        (root / 'dep').mkdir(parents=True)
        (root / 'dep/value.sapp').write_text('let value=42;export value;\n')
        (root / 'pblang.toml').write_text('[package]\nname="lock-fixture"\nversion="1.0.0"\nentry="main.sapp"\n[dependencies]\ndep={path="dep"}\n')
        (root / 'main.sapp').write_text('let D=import("dep:value.sapp");print D.value;\n')
        proc = run(['build', root], root)
        record('native project lock builds', proc.returncode == 0, stderr=proc.stderr)
        driver = harness / 'probe.sapp'
        driver.write_text('let L=import("stdlib:portable_lock.sapp");print json_stringify(L.publish(' + json.dumps(str(root)) + '));')
        proc = run(['run', driver, '--allow-all'], harness)
        record('native lock made portable', proc.returncode == 0, stderr=proc.stderr)
        manifest = (root / 'pblang.toml').read_bytes()
        lock = (root / 'pblang.lock').read_text()
        parsed = tomllib.loads(lock)
        digest = parsed['dependencies']['dep']['sha256']
        manifest_digest = parsed['manifest_sha256']
        zeros = '0' * 64
        driver.write_text('let S=import("stdlib:locked_source_snapshot.sapp");let B=import("stdlib:immutable_broker_bundle.sapp");let R=' + json.dumps(str(root)) + ';let s=S.load(R);let b=B.build("main.sapp",R);print json_stringify(map{snapshot:s.ok,bundle:b.ok,error:try s.error catch nil,bundle_error:try b.error catch nil,verified:try s.lock_verified catch false});')

        def case(label, text, accepted, manifest_bytes=manifest):
            (root / 'pblang.toml').write_bytes(manifest_bytes)
            (root / 'pblang.lock').write_bytes(text.encode() if isinstance(text, str) else text)
            before = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in root.rglob('*') if p.is_file()}
            proc = run(['run', driver, '--allow-all'], harness)
            lines = [line for line in proc.stdout.splitlines() if line.startswith('{')]
            result = json.loads(lines[-1]) if lines else {}
            after = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in root.rglob('*') if p.is_file()}
            record(label, proc.returncode == 0 and result.get('snapshot') == accepted
                   and result.get('bundle') == accepted and result.get('verified') == accepted
                   and before == after, result=result, source_unchanged=before == after, stderr=proc.stderr)

        case('genuine portable native lock', lock, True)
        flawed = [
            ('duplicate manifest digest', lock.replace('manifest_sha256 = ', 'manifest_sha256 = "' + zeros + '"\nmanifest_sha256 = ', 1)),
            ('duplicate algorithm', lock.replace('content_hash_algorithm = ', 'content_hash_algorithm = "legacy"\ncontent_hash_algorithm = ', 1)),
            ('duplicate dependency digest', lock.replace('sha256 = "' + digest + '"', 'sha256 = "' + zeros + '"\nsha256 = "' + digest + '"')),
            ('duplicate dependency path', lock.replace('path = "dep"', 'path = "wrong"\npath = "dep"')),
            ('undeclared extra dependency', lock + '\n[dependencies.extra]\npath="dep"\nsha256="' + digest + '"\n'),
            ('mixed registry and local record', lock + '\nsource="registry:example"\n'),
        ]
        for name, bad in flawed:
            case(('baseline accepts ' if opts.baseline else 'reject ') + name, bad, bool(opts.baseline))
        if opts.baseline:
            return
        case('comments and reordered fields', '# generated fixture\n' + lock.replace('path = "dep"\nsha256 = "' + digest + '"', 'sha256 = "' + digest + '" # digest\npath = "dep"'), True)
        case('quoted dependency fields', lock.replace('path = ', '"path" = ').replace('sha256 = "' + digest, '"sha256" = "' + digest), True)
        case('identical duplicate dependency digest', lock + '\nsha256="' + digest + '"\n', False)
        case('repeated dependency table', lock + '\n[dependencies.dep]\npath="dep"\nsha256="' + digest + '"\n', False)
        case('malformed line', lock + '\nnot-an-assignment\n', False)
        case('scalar versus table conflict', lock.replace('[dependencies.dep]', 'dependencies="wrong"\n[dependencies.dep]'), False)
        case('unsupported inline table lock', lock + '\nextra={path="unused"}\n', False)
        case('source marker empty still not local record', lock + '\nsource=""\n', False)
        case('renamed dependency same cardinality', lock.replace('[dependencies.dep]', '[dependencies.other]'), False)
        case('missing dependency', lock.split('[dependencies.dep]')[0] + '[dependencies]\n', False)
        case('invalid UTF8 lock', lock.encode() + b'\n#\xff\n', False)
        case('invalid UTF8 manifest', lock.replace(manifest_digest, hashlib.sha256(manifest + b'#\xff\n').hexdigest()), False, manifest + b'#\xff\n')
        case('stale valid dependency digest', lock.replace(digest, zeros), False)
        # No dependencies remains a valid project profile.
        empty_manifest = b'[package]\nname="empty"\nversion="1.0.0"\nentry="main.sapp"\n'
        (root / 'main.sapp').write_text('print 42;\n')
        empty_lock = 'manifest_sha256="' + hashlib.sha256(empty_manifest).hexdigest() + '"\ncontent_hash_algorithm="pblang-package-tree-v2"\n[dependencies]\n'
        case('empty dependency set', empty_lock, True, empty_manifest)
        case('unused locked dependency in empty project', empty_lock + '[dependencies.extra]\npath="dep"\nsha256="' + digest + '"\n', False, empty_manifest)


if __name__ == '__main__':
    main()
