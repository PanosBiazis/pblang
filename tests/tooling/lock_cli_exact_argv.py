"""Exercise lock maintenance CLIs against disposable Linux project fixtures.

No live installation is changed. The selected retained launcher and native binary
are read-only; the only writable mount is a new temporary fixture directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import tempfile
import time
import tomllib


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def limits():
    resource.setrlimit(resource.RLIMIT_AS, (768 * 1024**2, 768 * 1024**2))
    resource.setrlimit(resource.RLIMIT_FSIZE, (1024**2, 1024**2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--tools-dir', type=Path)
    parser.add_argument('--stdlib', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    opt = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    tools = (opt.tools_dir or repo / 'tools').resolve()
    stdlib = (opt.stdlib or repo / 'stdlib').resolve()
    exe = opt.binary.resolve()
    release = exe.parent.parent
    records = []
    setup = []
    tree = [(str(path.relative_to(stdlib)), digest(path))
            for path in sorted(stdlib.rglob('*')) if path.is_file()]
    tree_digest = hashlib.sha256(json.dumps(tree, separators=(',', ':')).encode()).hexdigest()
    report = dict(schema='pblang.lock-cli-argv.v1', cases=records, setup=setup,
                  harness_sha256=digest(Path(__file__)),
                  stdlib_tree_sha256=tree_digest, stdlib_file_count=len(tree),
                  launcher_sha256=digest(exe),
                  native_sha256=digest(exe.parent / 'pblang-native'),
                  tools_sha256={name: digest(tools / name) for name in
                                ['pblock.sapp', 'pblock_install.sapp']})
    opt.report.parent.mkdir(parents=True, exist_ok=True)
    # Refuse overwriting any prior run; failed runs are evidence too.
    with opt.report.open('x') as output:
        json.dump(report, output, indent=2)
    with tempfile.TemporaryDirectory(prefix='lock-cli-argv-') as folder:
        root = Path(folder)
        runner = root / 'runner'
        (runner / 'tools').mkdir(parents=True)
        shutil.copytree(stdlib, runner / 'stdlib')
        for name in report['tools_sha256']:
            shutil.copy2(tools / name, runner / 'tools' / name)
        (runner / 'main.sapp').write_text('print 0;\n')
        (runner / 'pblang.toml').write_text(
            '[package]\nname="lock-cli-runner"\nversion="0.1.0"\nentry="main.sapp"\n'
            '[dependencies]\nstdlib={path="stdlib"}\n')
        box = ['bwrap', '--unshare-all', '--die-with-parent',
               '--ro-bind', '/usr', '/usr', '--ro-bind', '/bin', '/bin',
               '--ro-bind', '/lib', '/lib', '--ro-bind', '/lib64', '/lib64',
               '--ro-bind', '/etc/ld.so.cache', '/etc/ld.so.cache',
               '--ro-bind', release, release, '--proc', '/proc', '--dev', '/dev',
               '--tmpfs', '/tmp', '--bind', root, root]
        environment = {key: value for key, value in os.environ.items()
                       if not key.startswith('PBLANG_')}
        environment.update(HOME=str(root), TMPDIR=str(root))

        def run(argv, cwd=runner, injected=None):
            started = time.monotonic()
            command = list(map(str, box + ['--chdir', cwd, exe] + argv))
            child = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, start_new_session=True,
                                     preexec_fn=limits(), env=environment | (injected or {}))
            try:
                out, err = child.communicate(timeout=40)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                out, err = child.communicate()
                raise RuntimeError('lock CLI timed out')
            finally:
                # All children in this invocation are disposable fixture work.
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            # Post-capture sanity check for controlled fixture programs; this is
            # not a streaming allocation bound for arbitrary hostile output.
            if len(out) + len(err) > 65536:
                raise RuntimeError('unexpected lock CLI output volume')
            return dict(exit=child.returncode, stdout=out.decode(errors='replace'),
                        stderr=err.decode(errors='replace'),
                        seconds=round(time.monotonic() - started, 4))

        built = run(['build'])
        setup.append(dict(step='isolated runner build', **built))
        if built['exit'] != 0:
            raise RuntimeError('runner build failed: ' + built['stderr'])
        original_runner_lock = (runner / 'pblang.lock').read_bytes()

        def fixture(name, portable):
            project = root / name
            (project / 'dep').mkdir(parents=True)
            (project / 'dep' / 'probe.sapp').write_text('print 7;\n')
            manifest = ('[package]\nname="fixture"\nversion="0.1.0"\nentry="main.sapp"\n'
                        '[dependencies]\ndep={path="dep"}\n')
            (project / 'pblang.toml').write_text(manifest)
            (project / 'main.sapp').write_text('print 0;\n')
            path = 'dep' if portable else str(project / 'dep')
            # The maintenance writer validates/preserves digests; it does not
            # certify dependency content. Synthetic valid-shaped hashes suffice.
            lock = ('content_hash_algorithm = "pblang-package-tree-v2"\n'
                    'manifest_sha256 = "' + hashlib.sha256(manifest.encode()).hexdigest() + '"\n'
                    '[dependencies.dep]\npath = ' + json.dumps(path) + '\nsha256 = "' + 'a'*64 + '"\n')
            (project / 'pblang.lock').write_text(lock)
            return project

        def snapshot():
            return {str(path.relative_to(root)): path.read_bytes()
                    for path in root.rglob('*') if path.is_file()}

        def check(tool, label, args, target=None, reject=False, env=None, cwd=runner,
                  separator=True, marker=None):
            before = snapshot()
            cmd = ['run', runner / 'tools' / (tool + '.sapp'), '--allow-all']
            if separator:
                cmd += ['--'] + list(map(str, args))
            result = run(cmd, cwd=cwd, injected=env)
            after = snapshot()
            changes = sorted(path for path in before.keys() | after.keys()
                             if before.get(path) != after.get(path))
            ok = (result['exit'] != 0 and not changes if reject else result['exit'] == 0)
            if not reject:
                expected = str((target / 'pblang.lock').relative_to(root))
                ok = ok and changes == [expected]
                if result['exit'] == 0:
                    parsed = tomllib.loads((target / 'pblang.lock').read_text())
                    old = tomllib.loads(before[expected].decode())
                    dependency = 'stdlib' if target == runner else 'dep'
                    wanted = dependency if tool == 'pblock' else str(target / dependency)
                    ok = ok and parsed['dependencies'][dependency]['path'] == wanted
                    old['dependencies'][dependency]['path'] = wanted
                    ok = ok and parsed == old
            if marker is not None:
                ok = ok and marker in result['stderr']
            records.append(dict(tool=tool, case=label, passed=ok,
                                changed_paths=changes, **result))
            # A baseline failure can alter the runner default lock. Restore only
            # this disposable copy before evaluating the next independent case.
            (runner / 'pblang.lock').write_bytes(original_runner_lock)
            opt.report.write_text(json.dumps(report, indent=2) + '\n')

        for tool in ['pblock', 'pblock_install']:
            install = tool == 'pblock_install'
            for suffix in ['ordinary', 'trailing ', ' leading', 'space inside', '--option']:
                project = fixture(tool + '-' + suffix, install)
                if suffix == 'trailing ':
                    fixture((tool + '-' + suffix).rstrip(), install)
                check(tool, suffix, [project], target=project)
            target = fixture(tool + '-arity-target', install)
            check(tool, 'extra_arg', [target, 'extra'], reject=True,
                  marker=tool + ':')
            check(tool, 'empty_arg', [''], reject=True, marker=tool + ':')
            # Environment must not choose a target in either direct invocation.
            decoy = fixture(tool + '-environment-decoy', install)
            check(tool, 'environment_injection', [], target=runner if not install else None,
                  reject=install, env={'PBLANG_SCRIPT_ARGS': str(decoy)}, separator=False,
                  marker=tool + ':' if install else None)
            target = fixture(tool + '-explicit-target', install)
            check(tool, 'explicit_arg_over_environment', [target], target=target,
                  env={'PBLANG_SCRIPT_ARGS': str(decoy)})
            check(tool, 'no_argument', [], target=runner if not install else None,
                  reject=install, separator=False, marker=tool + ':' if install else None)
            check(tool, 'separator_without_argument', [], target=runner if not install else None,
                  reject=install, marker=tool + ':' if install else None)
            for control in ['\n', '\r']:
                base = tool + '-control-' + ('lf' if control == '\n' else 'cr')
                fixture(base, install)  # Prefix decoy: old newline splitting hits it.
                target = fixture(base + control + 'tail', install)
                check(tool, 'refuse_' + ('newline' if control == '\n' else 'carriage_return'),
                      [target], reject=True, marker=tool + ':')
        report.update(passed=sum(row['passed'] for row in records), total=len(records),
                      failed=sum(not row['passed'] for row in records),
                      fixture_removed_after_run=True)
    opt.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: report[key] for key in ['passed', 'total', 'failed']}))
    raise SystemExit(1 if report['failed'] else 0)


if __name__ == '__main__':
    main()
