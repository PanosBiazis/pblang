"""Source-hidden native imported-document coverage, focus and reading-anchor recovery."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time

DRIVER = r'''
import json, os, subprocess, time
from pathlib import Path
state_path=Path('/probe/state.json'); observations=[]
child=subprocess.Popen(['/app/pblang','run','/probe/fixture.sapp','--allow-all','--deny-exec','--deny-net','--deny-http','--deny-spawn'],cwd='/app',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=4).strip()
def settled(**expected):
    end=time.monotonic()+7
    while time.monotonic()<end:
        if child.poll() is not None:raise AssertionError(child.communicate())
        try:
            s=json.loads(state_path.read_text())
            if all(s.get(k)==v for k,v in expected.items()):observations.append(s);return s
        except (FileNotFoundError,json.JSONDecodeError):pass
        time.sleep(.06)
    raise AssertionError((expected, state_path.read_text()))
def press(k):x('keydown',k);time.sleep(.18);x('keyup',k);time.sleep(.2)
def shot(name):
    # State is emitted during draw, before the window presents that frame.
    time.sleep(.15)
    subprocess.run(['import','-window',window,'/probe/'+name+'.png'],check=True,timeout=6)
try:
    end=time.monotonic()+10;window=None
    while time.monotonic()<end:
        p=subprocess.run(['xdotool','search','--name','^PBLang large imported reader$'],capture_output=True,text=True)
        if p.returncode==0:window=p.stdout.splitlines()[0];break
        if child.poll() is not None:raise AssertionError(child.communicate())
        time.sleep(.1)
    assert window; x('windowfocus','--sync',window)
    settled(width=320,height=240,available=False,bounds=True);shot('large-small-help')
    x('windowsize',window,320,480)
    if os.environ.get('READER_BASELINE')=='1':
        s=settled(width=320,height=480,available=False,bounds=True);shot('large-baseline-unavailable')
        assert s['reason']=='Text cluster cannot fit the supported viewport.'
        press('Escape');out,err=child.communicate(timeout=5);assert child.returncode==0,(out,err)
        print(json.dumps(dict(expected_failure_reproduced=True,observations=observations)))
    else:
        first=settled(width=320,height=480,available=True,page=0,bounds=True,exact=True);shot('large-first')
        count=int(first['pages']);assert count>20
        for page in range(1,count):press('Next');settled(page=page,bounds=True,exact=True)
        last=settled(page=count-1,bounds=True,exact=True);anchor=last['anchor'];shot('large-last')
        for _ in range(7):
            if json.loads(state_path.read_text())['focus']=='appearance':break
            press('Tab')
        settled(focus='appearance',focus_visible=True);press('Return');settled(settings=True,anchor=anchor,bounds=True)
        # Done is always the fourth compact toolbar control, independent of text magnification.
        x('mousemove','--window',window,275,32);x('mousedown','1');time.sleep(.18);x('mouseup','1')
        settled(settings=False,anchor=anchor,focus='appearance',focus_visible=True)
        for w,h,available in [(640,720,True),(320,240,False),(320,480,True)]:
            x('windowsize',window,w,h);settled(width=w,height=h,available=available,anchor=anchor,bounds=True,exact=True,focus='appearance',focus_visible=True)
        shot('large-restored');press('Prior');settled(page=count-2,bounds=True,exact=True);press('Next');settled(page=count-1,bounds=True,exact=True)
        for _ in range(7):
            if json.loads(state_path.read_text())['focus']=='block2':break
            press('Tab')
        settled(focus='block2',focus_visible=True);shot('large-link-focus');press('Return');settled(url='pb://about',bounds=True,exact=True)
        press('Escape');out,err=child.communicate(timeout=5)
        assert child.returncode==0 and 'reader_large_document_native_pass' in out,(out,err)
        print(json.dumps(dict(passed=True,visited_pages=count,all_imported_characters_drawn=True,anchor_recovered=True,style_roundtrip=True,focus_visible=True,final_link_opened=True,observations=observations)))
finally:
    if child.poll() is None:
        child.terminate()
        try:child.communicate(timeout=3)
        except subprocess.TimeoutExpired:child.kill();child.communicate()
'''

def snapshot(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob('*') if p.is_file()}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--package',type=Path,required=True)
    p.add_argument('--evidence',type=Path,required=True)
    p.add_argument('--baseline',action='store_true');a=p.parse_args()
    package=a.package.resolve();repo=Path(__file__).resolve().parents[2]
    evidence=a.evidence.absolute();evidence.mkdir(mode=0o700);before=snapshot(package)
    with tempfile.TemporaryDirectory(prefix='reader-large-') as temp:
        probe=Path(temp)
        fixture=(repo/'tests/fixtures/reader_large_document_native.sapp').read_text().replace('../../examples/declarative_browser/','/app/examples/declarative_browser/')
        (probe/'fixture.sapp').write_text(fixture);(probe/'driver.py').write_text(DRIVER)
        manifest='[package]\nname="reader-large-test"\nversion="1.0.0"\nentry="fixture.sapp"\n[dependencies]\nstdlib={path="/app/stdlib"}\n'
        (probe/'pblang.toml').write_text(manifest)
        lock=re.sub(r'manifest_sha256\s*=\s*"[a-f0-9]+"',f'manifest_sha256="{hashlib.sha256(manifest.encode()).hexdigest()}"',(package/'pblang.lock').read_text())
        (probe/'pblang.lock').write_text(re.sub(r'path\s*=\s*"stdlib"','path="/app/stdlib"',lock))
        cmd=['bwrap','--unshare-all','--die-with-parent','--ro-bind','/usr','/usr','--ro-bind','/bin','/bin','--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64','--ro-bind','/etc/fonts','/etc/fonts','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind',str(package),'/app','--bind',str(probe),'/probe','--chdir','/app','--clearenv','--setenv','PATH','/usr/bin:/bin','--setenv','HOME','/tmp','--setenv','LANG','C.UTF-8','--setenv','READER_BASELINE','1' if a.baseline else '0']
        for system in ('/etc/alternatives','/etc/ImageMagick-6','/etc/ImageMagick-7'):
            if Path(system).is_dir():cmd+=['--ro-bind',system,system]
        start=time.monotonic()
        r=subprocess.run(cmd+['/bin/sh','-c','test ! -e "$1" && test ! -e /home && exec xvfb-run -a -s "-screen 0 1000x1000x24 -nolisten tcp" env XMODIFIERS=@im=none python3 /probe/driver.py','sh',str(repo)],capture_output=True,text=True,timeout=50)
        (evidence/'native.log').write_text(r.stdout+r.stderr)
        for f in probe.glob('*.png'):shutil.copy2(f,evidence/f.name)
        assert r.returncode==0,(r.stdout,r.stderr)
        result=json.loads(r.stdout);result.update(source_hidden=True,package_unchanged=snapshot(package)==before,elapsed_seconds=time.monotonic()-start,package=str(package))
        assert result['package_unchanged'];(evidence/'acceptance.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='observations'}))

if __name__=='__main__':main()
