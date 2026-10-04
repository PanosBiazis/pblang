"""Accept a reader candidate on Linux with its original checkout inaccessible."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


LAUNCH_PROBE = '''import subprocess,time,sys
launch=sys.argv[1] if len(sys.argv)>1 else '/app/run-reader.sh'
child=subprocess.Popen(['/bin/sh',launch],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
try:
    window=None;deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        if child.poll() is not None:raise AssertionError(child.communicate())
        found=subprocess.run(['xdotool','search','--name','^PBLang document browser$'],capture_output=True,text=True,timeout=2)
        if found.returncode==0 and found.stdout.strip():window=found.stdout.splitlines()[0];break
        time.sleep(.1)
    assert window is not None,'Standalone reader window did not appear'
    time.sleep(.5)
    subprocess.run(['import','-window',window,'/probe/reader-standalone.png'],check=True,timeout=8)
    subprocess.run(['xdotool','windowfocus','--sync',window],check=True,timeout=5)
    def key(name):
        subprocess.run(['xdotool','keydown',name],check=True,timeout=5)
        time.sleep(.18)
        subprocess.run(['xdotool','keyup',name],check=True,timeout=5)
        time.sleep(.25)
    def click(x,y):
        subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'mousedown','1'],check=True,timeout=5)
        time.sleep(.18)
        subprocess.run(['xdotool','mouseup','1'],check=True,timeout=5)
        time.sleep(.3)
    click(700,44)
    for name in ('Return','Tab','Return','Return','Tab','Return','Return','Tab','Return'):key(name)
    subprocess.run(['import','-window',window,'/probe/reader-style.png'],check=True,timeout=8)
    click(700,44)
    subprocess.run(['xdotool','windowsize',window,'320','480'],check=True,timeout=5)
    time.sleep(.6)
    subprocess.run(['import','-window',window,'/probe/reader-first.png'],check=True,timeout=8)
    key('End')
    subprocess.run(['import','-window',window,'/probe/reader-last.png'],check=True,timeout=8)
    key('Escape')  # First Escape clears the focused Style/navigation control.
    if child.poll() is None:key('Escape')
    output,error=child.communicate(timeout=10)
    assert child.returncode==0,(output,error)
    print('standalone_window_pass')
finally:
    if child.poll() is None:
        child.terminate()
        try:child.communicate(timeout=3)
        except subprocess.TimeoutExpired:child.kill();child.communicate()
'''


def snapshot(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', required=True, type=Path)
    parser.add_argument('--evidence', required=True, type=Path, help='New evidence directory')
    args = parser.parse_args()
    package = args.package.resolve(); evidence = args.evidence.absolute()
    evidence.mkdir(mode=0o700)
    repo = Path(__file__).resolve().parents[2]
    before = snapshot(package)
    inventory = json.loads((package / 'reader-source-sha256.json').read_text())
    assert all(before[name] == digest for name, digest in inventory.items())
    assert all(hashlib.sha256((repo/name).read_bytes()).hexdigest() == digest for name,digest in inventory.items())
    receipt = json.loads((package/'package-receipt.json').read_text())
    assert receipt['complete'] and not receipt['native_bundle_invoked']
    assert str(repo) not in (package/'pblang.lock').read_text()
    results = []
    def record(name, passed, **details):
        results.append(dict(name=name,passed=passed,**details))
        (evidence/'acceptance.json').write_text(json.dumps(results,indent=2))
        print(name, passed, flush=True)
        assert passed, details
    record('candidate module identities', True, modules=len(inventory), files=len(before),runtime_sha256=before['pblang'])
    refused = subprocess.run([str(package/'pblang'),'run',str(repo/'examples/declarative_browser/package.sapp'),
                             '--allow-all','--',str(repo),str(package),'--runtime',str(package/'pblang')],
                            cwd=repo,capture_output=True,text=True,timeout=30)
    (evidence/'existing-output.log').write_text(refused.stdout+refused.stderr)
    record('existing candidate refused unchanged',refused.returncode==1 and before==snapshot(package)
           and 'Output already exists' in refused.stdout)
    with tempfile.TemporaryDirectory(prefix='reader-existing-stage-') as temporary:
        destination=Path(temporary)/'candidate';stage=Path(str(destination)+'.source');stage.mkdir()
        (stage/'sentinel').write_text('preserve existing staging')
        refused=subprocess.run([str(package/'pblang'),'run',str(repo/'examples/declarative_browser/package.sapp'),
                                '--allow-all','--',str(repo),str(destination),'--runtime',str(package/'pblang')],
                               cwd=repo,capture_output=True,text=True,timeout=30)
        record('existing staging refused unchanged',refused.returncode==1 and 'Staging already exists' in refused.stdout
               and not destination.exists() and (stage/'sentinel').read_text()=='preserve existing staging'
               and len(list(stage.iterdir()))==1)
    with tempfile.TemporaryDirectory(prefix='reader-accept-') as temporary:
        probe = Path(temporary)
        # Test-only fixture and driver. Application modules and runtime are read
        # directly from the read-only candidate, never from the source checkout.
        fixture = (repo/'tests/fixtures/reader_text_fit_native.sapp').read_text()
        fixture = fixture.replace('../../examples/declarative_browser/', '/app/examples/declarative_browser/')
        fixture = fixture.replace('fs_write("build/', 'fs_write("/probe/build/')
        (probe/'native.sapp').write_text(fixture)
        semantic = (repo/'tests/semantics/reader_text_fit_test.sapp').read_text()
        (probe/'semantic.sapp').write_text(semantic.replace('../../examples/declarative_browser/', '/app/examples/declarative_browser/'))
        shutil.copy2(repo/'tests/tooling/reader_text_fit_native.py', probe/'driver.py')
        (probe/'launch.py').write_text(LAUNCH_PROBE)
        manifest = ('[package]\nname="reader-acceptance"\nversion="1.0.0"\nentry="native.sapp"\n'
                    '[dependencies]\nstdlib={path="/app/stdlib"}\n')
        (probe/'pblang.toml').write_text(manifest)
        lock = (package/'pblang.lock').read_text()
        import re
        lock = re.sub(r'manifest_sha256\s*=\s*"[a-f0-9]+"',
                      f'manifest_sha256="{hashlib.sha256(manifest.encode()).hexdigest()}"',lock)
        lock = re.sub(r'path\s*=\s*"stdlib"','path="/app/stdlib"',lock)
        (probe/'pblang.lock').write_text(lock)
        sandbox = ['bwrap','--unshare-all','--die-with-parent',
                   '--ro-bind','/usr','/usr','--ro-bind','/bin','/bin',
                   '--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64',
                   '--ro-bind','/etc/fonts','/etc/fonts','--proc','/proc','--dev','/dev',
                   '--tmpfs','/tmp','--dir','/tmp/foreign','--ro-bind',str(package),'/app',
                   '--bind',str(probe),'/probe','--chdir','/tmp/foreign','--clearenv',
                   '--setenv','PATH','/usr/bin:/bin','--setenv','HOME','/tmp',
                   '--setenv','LANG','C.UTF-8']
        # Debian-family command symlinks and ImageMagick's system configuration
        # are OS requirements; no home or source checkout is exposed.
        for system in ('/etc/alternatives','/etc/ImageMagick-6','/etc/ImageMagick-7'):
            if Path(system).is_dir():
                sandbox.extend(['--ro-bind',system,system])
        def run(name, command, timeout=60, success=0, marker=None):
            result = subprocess.run(sandbox+command,capture_output=True,text=True,timeout=timeout)
            (evidence/(name+'.log')).write_text(result.stdout+result.stderr)
            for screenshot in probe.glob('reader-*.png'):
                shutil.copy2(screenshot,evidence/screenshot.name)
            record(name,result.returncode==success and (marker is None or marker in result.stdout),exit=result.returncode)
        run('checkout absent',['/bin/sh','-c','test ! -e "$1" && test ! -e /home && test ! -e /media','sh',str(repo)])
        run('foreign cwd CLI',['/bin/sh','/app/run-reader.sh','--check'],marker='declarative_browser_pass')
        run('packaged semantic',['/bin/sh','-c','cd /app && exec "$@"','sh',
                                 '/app/pblang','run','/probe/semantic.sapp','--deny-exec','--deny-ffi',
                                 '--deny-net','--deny-fs','--deny-http','--deny-sqlite','--deny-spawn',
                                 '--deny-gfx','--deny-env','--allow-import'],marker='reader_text_fit_pass checks=855')
        run('unsupported launch argument',['/bin/sh','/app/run-reader.sh','--export'],success=2)
        run('native launcher',['xvfb-run','-a','-s','-screen 0 1000x1000x24 -nolisten tcp',
                              'env','XMODIFIERS=@im=none','python3','/probe/launch.py'],
            timeout=45,marker='standalone_window_pass')
        run('packaged text fit',['xvfb-run','-a','-s','-screen 0 1000x1000x24 -nolisten tcp',
                                'env','XMODIFIERS=@im=none','python3','/probe/driver.py',
                                '--runtime','/app/pblang','--fixture','/probe/native.sapp','--work-dir','/probe','--cwd','/app'],
            timeout=90,marker='"result": "pass"')
        for screenshot in (probe/'build').glob('reader-measured-*.png'):
            shutil.copy2(screenshot,evidence/screenshot.name)
        for screenshot in probe.glob('reader-*.png'):
            shutil.copy2(screenshot,evidence/screenshot.name)
    record('candidate unchanged after acceptance',before==snapshot(package))


if __name__ == '__main__':
    main()
