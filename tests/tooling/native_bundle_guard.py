"""Linux public-launcher contract. Native destructive repro uses disposable trees only."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile


def tree(root):
    result = {}
    for p in sorted(root.rglob('*')):
        key = p.relative_to(root).as_posix()
        if p.is_symlink():
            result[key] = ['link', os.readlink(p)]
        elif p.is_file():
            result[key] = ['file', hashlib.sha256(p.read_bytes()).hexdigest()]
        elif p.is_dir():
            result[key] = ['dir']
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--native', required=True)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--report', required=True)
    parser.add_argument('--launcher', help='Also qualify this installed public entry')
    parser.add_argument('--reproduce-native', action='store_true')
    opts = parser.parse_args()
    native = Path(opts.native).resolve()
    repo = Path(opts.repo).resolve()
    rows = []

    def record(name, passed, **details):
        row = dict(name=name, passed=bool(passed), **details)
        rows.append(row)
        Path(opts.report).write_text(json.dumps(rows, indent=2, ensure_ascii=False))
        print(name, bool(passed), flush=True)
        assert passed, row

    with tempfile.TemporaryDirectory(prefix='pblang-launcher-') as temp:
        base = Path(temp)
        release = base / 'release space ελληνικά'
        (release / 'bin').mkdir(parents=True)
        (release / 'libexec').mkdir()
        shutil.copy2(native, release / 'bin/pblang-native')
        shutil.copy2(repo / 'tools/pblang-launch.sh', release / 'bin/pblang')
        (release / 'bin/pblang').chmod(0o755)
        guard = release / 'libexec/native_bundle_guard.sapp'
        shutil.copy2(repo / 'tools/native_bundle_guard.sapp', guard)
        link = base / 'pblang'
        link.symlink_to(release / 'bin/pblang')
        # Mount only OS files and disposable fixtures. The original checkout is
        # absent; native input has its own read-only mount for the optional repro.
        sandbox = ['bwrap', '--unshare-all', '--die-with-parent']
        for name in ['/usr', '/bin', '/lib', '/lib64']:
            sandbox += ['--ro-bind', name, name]
        sandbox += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                    '--bind', str(base), str(base), '--ro-bind', str(native), str(native)]
        public = Path(opts.launcher).absolute() if opts.launcher else None
        if public:
            real = public.resolve()
            sandbox += ['--ro-bind', str(real.parent.parent), str(real.parent.parent)]
            if public != real:
                sandbox += ['--ro-bind', str(public.parent), str(public.parent)]

        def run(binary, args, cwd, env=None, stdin=''):
            return subprocess.run(sandbox + ['--chdir', str(cwd), str(binary)] + list(map(str, args)),
                                  text=True, input=stdin, capture_output=True, timeout=30,
                                  env=os.environ | (env or {}))

        def fixture(name):
            root = base / name
            root.mkdir()
            source = root / 'source'
            source.mkdir()
            (source / 'pblang.toml').write_text('[package]\nname="bundle-guard-fixture"\nversion="0.1.0"\nentry="main.sapp"\n')
            (source / 'main.sapp').write_text('print "bundle-integrity-canary";\n')
            return root, source

        if opts.reproduce_native:
            root, source = fixture('native-relative-baseline')
            before = (source / 'main.sapp').read_bytes()
            p = run(native, ['bundle', '.', root / 'out'], source)
            after = (source / 'main.sapp').read_bytes()
            record('retained native relative-root truncation reproduced',
                   p.returncode == 0 and len(before) == 33 and after == b'',
                   exit=p.returncode, source_bytes_before=len(before), source_bytes_after=len(after),
                   native_sha256=hashlib.sha256(native.read_bytes()).hexdigest(), stdout=p.stdout, stderr=p.stderr)

        entries = [('fixture', link)] + ([('public', public)] if public else [])
        for prefix, launcher in entries:
            for shape in ['sibling', 'self', 'descendant', 'ancestor', 'existing', 'output-link',
                          'parent-link', 'entry-alias', 'source-link', 'absolute', 'no-args',
                          'help', 'self-extract', 'capability-flags', 'separator', 'newline-output']:
                root, source = fixture(prefix + '-' + shape)
                out = root / 'out'
                source_arg = '.'
                extra = []
                if shape == 'self': out = source
                elif shape == 'descendant': out = source / 'out'
                elif shape == 'ancestor': out = root
                elif shape == 'existing':
                    out.mkdir(); (out / 'sentinel').write_text('preserve')
                elif shape == 'output-link': out.symlink_to(source)
                elif shape == 'parent-link':
                    (root / 'alias').symlink_to(source); out = root / 'alias/out'
                elif shape == 'entry-alias': out.symlink_to(source / 'main.sapp')
                elif shape == 'source-link':
                    (root / 'alias').symlink_to(source); source_arg = str(root / 'alias')
                elif shape == 'absolute': source_arg = str(source)
                elif shape == 'self-extract': extra = ['--self-extract']
                elif shape == 'capability-flags': extra = ['--allow-all', '--sandbox']
                elif shape == 'separator': extra = ['--']
                elif shape == 'newline-output': out = root / 'out\n--help'
                args = ['bundle', *extra, source_arg, str(out)]
                if shape == 'no-args': args = ['bundle']
                if shape == 'help': args = ['bundle', '--help']
                before = tree(root)
                p = run(launcher, args, source, {'PBLANG_ALLOW_ALL': '1', 'PBLANG_SCRIPT_ARGS': 'help'})
                record(prefix + ' denied ' + shape,
                       p.returncode != 0 and 'native bundle is disabled' in p.stderr and tree(root) == before,
                       exit=p.returncode, source_and_destination_unchanged=tree(root) == before)

            root, source = fixture(prefix + '-passthrough')
            script = source / 'args.sapp'
            script.write_text('print json_stringify(os_args());\n')
            failing = source / 'exit.sapp'
            failing.write_text('os_exit(17);\n')
            inputs = [('--version',), ('--help',), ('version',), ('help',),
                      ('run', str(script), '--', '', 'two words', 'ελληνικά', 'line\nnext', 'bundle', '--allow-all'),
                      (str(script), '--', 'bare-script'), ('run', str(failing)),
                      ('bundle\nhelp',), ('--', 'bundle'), ('--allow-all', 'bundle'), ('Bundle',),
                      ('run', '.', '--', 'bundle'), ('--version', 'bundle')]
            for i, args in enumerate(inputs):
                before = tree(root)
                expected = run(native, args, source)
                actual = run(launcher, args, source)
                record(prefix + ' native parity ' + str(i),
                       (actual.returncode, actual.stdout, actual.stderr) ==
                       (expected.returncode, expected.stdout, expected.stderr) and tree(root) == before,
                       args=list(args), exit=actual.returncode, stdout=actual.stdout, stderr=actual.stderr)
            # The no-argument REPL receives the original input, not the guard.
            expected = run(native, [], source, stdin='print "stdin-preserved";\n')
            actual = run(launcher, [], source, stdin='print "stdin-preserved";\n')
            record(prefix + ' no-argument stdin parity',
                   (actual.returncode, actual.stdout, actual.stderr) ==
                   (expected.returncode, expected.stdout, expected.stderr), stdout=actual.stdout)

        original_guard = guard.read_text()
        for label, contents in [('missing', None), ('empty', ''), ('syntax', 'let = ;'),
                                ('wrong-marker', 'print "yes";'),
                                ('attempted-import', 'let x=import("absent.sapp"); print "pblang-launch-admit-v1";')]:
            if contents is None: guard.unlink()
            else: guard.write_text(contents)
            p = run(link, ['--version'], base)
            record('guard failure ' + label, p.returncode != 0 and not p.stdout, exit=p.returncode, stderr=p.stderr)
        guard.write_text(original_guard)
        (release / 'bin/pblang-native').unlink()
        p = run(link, ['--version'], base)
        record('missing runtime fails closed', p.returncode == 126 and not p.stdout, stderr=p.stderr)
    print('ALL PASS', len(rows), flush=True)


if __name__ == '__main__':
    main()
