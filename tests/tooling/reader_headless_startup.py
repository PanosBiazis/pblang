"""Prove headless Reader still validates navigation without loading its GUI app.

The real package and source remain read-only. A private mount overlays synthetic
faults only for individual subprocesses, never installed files or permissions.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def snapshot(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path, required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    package = args.package.resolve()
    evidence = args.evidence.absolute()
    evidence.mkdir(mode=0o700)
    before = snapshot(package)
    results = []
    with tempfile.TemporaryDirectory(prefix='reader-headless-startup-') as temporary:
        probe = Path(temporary)
        marker = 'reader_presentation_fixture_loaded'
        (probe/'app.sapp').write_text('throw "'+marker+'";\n')
        pages = (package/'examples/declarative_browser/pages.sapp').read_text()
        needle = r'href=\"pb://guide\"'
        assert pages.count(needle) == 1
        (probe/'wrong-pages.sapp').write_text(pages.replace(needle, r'href=\"pb://about\"'))
        sandbox = ['bwrap', '--unshare-all', '--die-with-parent',
                   '--ro-bind', '/usr', '/usr', '--ro-bind', '/bin', '/bin',
                   '--ro-bind', '/lib', '/lib', '--ro-bind', '/lib64', '/lib64',
                   '--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
                   '--ro-bind', str(package), '/app', '--bind', str(probe), '/probe',
                   '--chdir', '/probe', '--clearenv', '--setenv', 'PATH', '/usr/bin:/bin',
                   '--setenv', 'HOME', '/tmp', '--setenv', 'LANG', 'C.UTF-8']
        def run(name, command, fault=None, success=True, expected=''):
            mount = [] if fault is None else ['--ro-bind', str(probe/fault[0]), '/app/examples/declarative_browser/'+fault[1]]
            p = subprocess.run(sandbox+mount+command, capture_output=True, text=True, timeout=15)
            (evidence/(name+'.log')).write_text(p.stdout+p.stderr)
            passed = (p.returncode == 0) == success and expected in p.stdout+p.stderr
            results.append(dict(name=name, passed=passed, exit=p.returncode))
            (evidence/'acceptance.json').write_text(json.dumps(results, indent=2))
            assert passed, (name,p.stdout,p.stderr)
        run('headless-normal', ['/bin/sh','/app/run-reader.sh','--check'], expected='declarative_browser_pass')
        fault = ('app.sapp','app.sapp')
        run('headless-with-unavailable-presentation', ['/bin/sh','/app/run-reader.sh','--check'], fault, expected='declarative_browser_pass')
        run('gui-still-loads-presentation', ['/bin/sh','/app/run-reader.sh','--session-only'], fault, success=False, expected=marker)
        run('navigation-check-still-rejects-wrong-link', ['/bin/sh','/app/run-reader.sh','--check'], ('wrong-pages.sapp','pages.sapp'), success=False, expected='local navigation')
        assert before == snapshot(package)
        results.append(dict(name='package-unchanged', passed=True))
        (evidence/'acceptance.json').write_text(json.dumps(results, indent=2))
    print(json.dumps(dict(passed=True, checks=results, source_hidden=True, faults_private=True)))


if __name__ == '__main__':
    main()
