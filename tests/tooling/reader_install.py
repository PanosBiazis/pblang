"""Exercise PBLang installer transitions with synthetic receipts in a temporary root.

These test receipts exercise admission; they do not accept a real user release.
Run reader_package.py and review native Style/pagination before real activation.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import fcntl


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path, required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    package = args.package.resolve(); evidence = args.evidence.absolute()
    evidence.mkdir(mode=0o700)
    repo = Path(__file__).resolve().parents[2]
    pin = hashlib.sha256(base64.b64encode((package/'pblang').read_bytes())).hexdigest()
    results = []
    def record(name, ok, **details):
        results.append(dict(name=name, passed=bool(ok), **details))
        (evidence/'acceptance.json').write_text(json.dumps(results, indent=2))
        print(name, bool(ok), flush=True)
        assert ok, details
    with tempfile.TemporaryDirectory(prefix="reader-install-'test-") as tmp:
        work = Path(tmp); root = work/'reader'; bin_dir = work/'bin'; bin_dir.mkdir(mode=0o700)
        launcher = bin_dir/'pb-document-reader'
        def call(name, *command, success=True):
            result = subprocess.run(['/bin/sh', str(package/'run-reader.sh'), '--manage', *map(str, command)],
                                    cwd='/tmp', capture_output=True, text=True, timeout=60)
            (evidence/(name+'.log')).write_text(result.stdout+result.stderr)
            rows = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
            assert rows, result.stderr
            data = rows[-1]
            record(name, data['ok'] == success and (result.returncode == 0) == success
                   and data['owned_handles_closed'], error=data['error'])
            return data
        call('wrong runtime pin', 'prepare', package, root, 'v1', '0'*64, success=False)
        record('wrong pin made no root', not root.exists())
        foreign = work/'foreign'; foreign.mkdir(mode=0o700); (foreign/'sentinel').write_text('untouched')
        call('unmanaged root refused', 'prepare', package, foreign, 'v1', pin, success=False)
        record('unmanaged root preserved', sorted(p.name for p in foreign.iterdir()) == ['sentinel'])
        one = call('prepare v1', 'prepare', package, root, 'v1', pin)['release']
        sentinel = root/'data'/'keep.txt'; sentinel.write_text('user data stays')
        two = call('prepare v2', 'prepare', package, root, 'v2', pin)['release']
        call('existing version refused', 'prepare', package, root, 'v1', pin, success=False)
        call('verify v1', 'verify', root, 'v1')
        record('independent fingerprint', one['fingerprint'] == hashlib.sha256(
            ('pblang.reader.release.v1\nv1\n'+''.join(f'{k}={v}\n' for k,v in sorted(one['files'].items()))).encode()).hexdigest())
        def receipt(metadata):
            path = work/(metadata['version']+'.json')
            path.write_text(json.dumps(dict(format='pblang.reader.acceptance.v1', version=metadata['version'],
                fingerprint=metadata['fingerprint'], passed=True,
                checks={key:True for key in ('source_hidden','launcher','style','pagination','content_unchanged')},
                provenance='Synthetic TEST ONLY, not product acceptance')))
            return path
        r1 = receipt(one); r2 = receipt(two)
        wrong = work/'wrong.json'; wrong.write_text(r1.read_text().replace(one['fingerprint'], '0'*64))
        call('mismatched acceptance refused', 'activate', root, 'v1', bin_dir, wrong, success=False)
        record('no activation from wrong acceptance', not launcher.exists())
        launcher.write_text('#!/bin/sh\necho foreign\n'); original = launcher.read_bytes()
        call('foreign launcher refused', 'activate', root, 'v1', bin_dir, r1, success=False)
        record('foreign launcher preserved', launcher.read_bytes() == original)
        launcher.unlink()  # Test-owned fixture, never a user launcher.
        call('activate v1', 'activate', root, 'v1', bin_dir, r1)
        original = launcher.read_bytes()
        call('no rollback baseline refused', 'rollback', root, bin_dir, success=False)
        record('failed rollback preserves launcher', launcher.read_bytes() == original)
        with (root/'install.lock').open('r+') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            call('concurrent install refused', 'status', root, bin_dir, success=False)
        call('activate v2', 'activate', root, 'v2', bin_dir, r2)
        duplicate = call('same activation is no-op', 'activate', root, 'v2', bin_dir, r2)
        record('same activation preserves previous', not duplicate['selection_changed'] and duplicate['selection']['previous']=='v1')
        selected = call('rollback to v1', 'rollback', root, bin_dir)
        record('rollback selects accepted previous', selected['selection'] == dict(current='v1',previous='v2'))
        call('rollback to v2', 'rollback', root, bin_dir)
        damaged = root/'releases/v2/README.txt'; saved = damaged.read_bytes(); damaged.write_bytes(saved+b'changed')
        call('tampered release rejected', 'verify', root, 'v2', success=False)
        call('damaged current can roll back', 'rollback', root, bin_dir)
        original = launcher.read_bytes()
        call('damaged previous refused', 'rollback', root, bin_dir, success=False)
        record('failed selection remains unchanged', launcher.read_bytes() == original)
        damaged.write_bytes(saved)
        call('restored previous verified', 'verify', root, 'v2')
        call('return to v2', 'rollback', root, bin_dir)
        extra = root/'releases/v2/unexpected.txt'; extra.write_text('extra'); extra.chmod(0o600)
        call('unexpected release file refused', 'verify', root, 'v2', success=False); extra.unlink()
        target = root/'releases/v2/README.txt'; target.unlink(); target.symlink_to(root/'releases/v1/README.txt')
        call('symlink release refused', 'verify', root, 'v2', success=False); target.unlink(); target.write_bytes(saved); target.chmod(0o600)
        sandbox = ['bwrap','--unshare-all','--die-with-parent','--ro-bind','/usr','/usr',
                   '--ro-bind','/bin','/bin','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64',
                   '--proc','/proc','--dev','/dev','--tmpfs','/tmp','--bind',str(root),str(root),
                   '--ro-bind',str(bin_dir),str(bin_dir),'--chdir','/tmp','--clearenv',
                   '--setenv','PATH','/usr/bin:/bin','--setenv','HOME','/tmp']
        result = subprocess.run(sandbox+['/bin/sh','-c','test ! -e "$1" && exec "$2" --manage status "$3" "$4"',
                                'sh',str(repo),str(launcher),str(root),str(bin_dir)],capture_output=True,text=True,timeout=60)
        (evidence/'source-hidden-manager.log').write_text(result.stdout+result.stderr)
        record('source hidden installed manager',result.returncode==0 and json.loads(result.stdout)['selection']['current']=='v2')
        check = subprocess.run([str(launcher),'--check'],cwd='/tmp',capture_output=True,text=True,timeout=30)
        record('installed launcher offline check',check.returncode==0 and 'declarative_browser_pass' in check.stdout)
        record('user data preserved',sentinel.read_text()=='user data stays' and list((root/'data').iterdir())==[sentinel])
        record('two complete releases retained',sorted(p.name for p in (root/'releases').iterdir())==['v1','v2'])
        record('private executable launcher',launcher.stat().st_mode & 0o777 == 0o700)


if __name__ == '__main__':
    main()
