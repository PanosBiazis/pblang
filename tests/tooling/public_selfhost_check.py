"""Linux public self-hosted checker contract; no user source is executed."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import tempfile


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--report', type=Path, required=True)
    ap.add_argument('--group', choices=['source', 'argv', 'permissions', 'scope'], required=True)
    opts = ap.parse_args()
    release = opts.candidate.resolve()
    binary = release / 'bin/pblang'
    native = release / 'bin/pblang-native'
    rows = []
    with tempfile.TemporaryDirectory(prefix='pblang-public-check-') as folder:
        root = Path(folder)
        sources = {
            'valid.sapp': 'let n=3;print n;',
            'syntax.sapp': 'let = ;',
            'type.sapp': 'let n:number="wrong";',
            'missing-name.sapp': 'print absent;',
            'arity.sapp': 'fn f(x:number){return x;} f();',
            'empty.sapp': '',
            'space.sapp': '  \n ',
            'comment.sapp': '// comment only',
            'lexical.sapp': 'let n=0x;',
            'unterminated.sapp': 'print "unfinished',
            'throw.sapp': 'throw "MUST_NOT_EXECUTE";',
            'effect.sapp': 'fs_write("must-not-exist", "MUST_NOT_EXECUTE");',
        }
        for name, source in sources.items():
            (root / name).write_text(source)
        (root / 'folder').mkdir()
        # Neither the source checkout nor a host device/network is available.
        sandbox = ['bwrap', '--unshare-all', '--die-with-parent']
        for d in ['/usr', '/bin', '/lib', '/lib64']:
            sandbox += ['--ro-bind', d, d]
        sandbox += ['--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                    '--ro-bind', str(release), str(release), '--bind', str(root), str(root),
                    '--chdir', str(root)]

        def run(exe, args, env=None):
            return subprocess.run(sandbox + [str(exe), *map(str, args)], input='',
                                  text=True, capture_output=True, timeout=10,
                                  env=os.environ | (env or {}))

        def record(name, args, expected=0, env=None, parity=False):
            before = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in root.iterdir() if p.is_file()}
            result = run(binary, args, env)
            after = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in root.iterdir() if p.is_file()}
            passed = (result.returncode == expected if expected is not None else result.returncode != 0)
            passed = passed and before == after
            if parity:
                old = run(native, args, env)
                passed = passed and (result.returncode, result.stdout, result.stderr) == (old.returncode, old.stdout, old.stderr)
            elif expected == 0:
                passed = passed and 'self-hosted check ok' in result.stdout
            row = dict(name=name, passed=passed, args=list(map(str, args)), exit=result.returncode,
                       stdout=result.stdout, stderr=result.stderr, sources_unchanged=before == after)
            rows.append(row)
            opts.report.write_text(json.dumps(rows, indent=2, ensure_ascii=False))
            print(name, passed, flush=True)
            assert passed, row

        if opts.group == 'source':
            for name in sources:
                valid = name in ['valid.sapp', 'throw.sapp', 'effect.sapp']
                record(name, ['--check', '--via-pblang', name], 0 if valid else 1)
            for name in ['absent.sapp', 'folder']:
                record(name, ['--check', '--via-pblang', name], 1)
        elif opts.group == 'argv':
            for alias in ['--via-pblang', '--self-host']:
                for i, args in enumerate([
                    ['--check', alias, 'valid.sapp'], [alias, '--check', 'valid.sapp'],
                    ['--check', 'valid.sapp', alias], [alias, 'valid.sapp', '--check'],
                    ['--deny-net', alias, '--check', 'valid.sapp'],
                    ['--check', alias, '--', 'valid.sapp'],
                ]):
                    record(alias + ' order ' + str(i), args)
            record('both aliases', ['--check', '--via-pblang', '--self-host', 'valid.sapp'])
            record('repeated alias', ['--check', '--via-pblang', '--via-pblang', 'valid.sapp'])
            for name in ['two words.sapp', 'ελληνικά.sapp', 'line\nnext.sapp', '--deny-fs', '--sandbox', '--via-pblang']:
                (root / name).write_text(sources['valid.sapp'])
                record('literal filename ' + repr(name), ['--check', '--via-pblang', '--', name])
            for i, args in enumerate([
                ['--check', '--via-pblang'], ['--check', '--via-pblang', ''],
                ['--check', '--via-pblang', 'valid.sapp', 'syntax.sapp'],
                ['--check', '--via-pblang', '--unknown', 'valid.sapp'],
                ['--check', '--via-pblang', 'valid.sapp', '--unknown'],
                ['--check', '--via-pblang', '--ffi-profile=absent', 'valid.sapp'],
                ['--check', '--via-pblang', '--', 'valid.sapp', '--deny-fs'],
                ['--check', '--', 'valid.sapp', '--via-pblang'],
                ['--check', '--check', '--via-pblang', 'valid.sapp'],
            ]):
                record('invalid argv ' + str(i), args, 2)
        elif opts.group == 'permissions':
            for flag in ['--deny-fs', '--deny-import', '--sandbox']:
                record(flag, ['--check', '--via-pblang', flag, 'valid.sapp'], None)
            for i, flags in enumerate([
                ['--deny-fs', '--allow-fs'], ['--deny-import', '--allow-import'],
                ['--deny-env'], ['--deny-exec', '--deny-ffi', '--deny-net', '--deny-http', '--deny-spawn', '--deny-gfx', '--deny-sqlite', '--deny-env']
            ]):
                record('permitted ' + str(i), ['--check', '--via-pblang', *flags, 'valid.sapp'])
            for i, flags in enumerate([
                ['--allow-all', '--deny-fs'], ['--allow-import', '--deny-import'],
                ['--allow-all', '--sandbox'], ['--sandbox', '--allow-all'],
            ]):
                record('denied ' + str(i), ['--check', '--via-pblang', *flags, 'valid.sapp'], None)
            for key in ['PBLANG_SANDBOX', 'PBLANG_DENY_IMPORT']:
                record('environment ' + key, ['--check', '--via-pblang', 'valid.sapp'], None, {key: '1'})
            record('environment deny env', ['--check', '--via-pblang', 'valid.sapp'], env={'PBLANG_DENY_ENV': '1'})
            record('env does not supply argv', ['--check', '--via-pblang', 'syntax.sapp'], 1,
                   {'PBLANG_SCRIPT_ARGS': 'valid.sapp', 'PBLANG_ALLOW_ALL': '1'})
        else:
            (root / 'args.sapp').write_text('print json_stringify(os_args());')
            for i, args in enumerate([
                ['--check', 'valid.sapp'], ['--check', 'syntax.sapp'],
                ['check', 'valid.sapp'], ['check', 'syntax.sapp'],
                ['--version'], ['--help'],
                ['run', 'args.sapp', '--', '--check', '--via-pblang', 'syntax.sapp'],
                ['args.sapp', '--', '--check', '--self-host'],
                ['--via-pblang'], ['--check'],
            ]):
                expected = run(native, args).returncode
                record('unchanged route ' + str(i), args, expected, parity=True)
            (root / 'decoy/compiler').mkdir(parents=True)
            (root / 'decoy/compiler/source_analysis.sapp').write_text('throw "DECOY_EXECUTED";')
            (root / 'pblang.toml').write_text('[package]\nname="caller"\nversion="0.1.0"\nentry="valid.sapp"\n[dependencies]\nstdlib={path="decoy"}\n')
            (root / 'pblang.lock').write_text('invalid caller lock')
            record('installed origin ignores caller decoy', ['--check', '--via-pblang', 'valid.sapp'])
    print('ALL PASS', len(rows), flush=True)


if __name__ == '__main__':
    main()
