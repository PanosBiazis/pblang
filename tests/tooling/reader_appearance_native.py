"""Source-hidden real reader GUI Save/Apply/Reset/restart journey on synthetic data.

The installed mode uses the real launcher and default data discovery, overlaying
only its data directory with a temporary private folder. Review the screenshots.
"""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path

PROBE=r'''import json,os,subprocess,time
from pathlib import Path
launch=os.environ['READER_TEST_LAUNCH'];data=Path(os.environ['READER_TEST_DATA']);profile=data/'reader-style/appearance-v1.json'
child=None;window=None

def x(*args):return subprocess.run(['xdotool',*map(str,args)],check=True,capture_output=True,text=True,timeout=5)
def key(name):
    x('keydown',name);time.sleep(.18);x('keyup',name);time.sleep(.22)
def click(xpos,ypos):
    x('mousemove','--window',window,xpos,ypos,'mousedown','1');time.sleep(.18);x('mouseup','1');time.sleep(.35)
def shot(name):subprocess.run(['import','-window',window,'/probe/'+name+'.png'],check=True,timeout=8)
def start(with_profile=True):
    global child,window
    child=subprocess.Popen(['/bin/sh',launch]+(['--with-profile'] if with_profile else []),stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    end=time.monotonic()+15;window=None
    while time.monotonic()<end:
        if child.poll() is not None:raise AssertionError(child.communicate())
        found=subprocess.run(['xdotool','search','--name','^PBLang document browser$'],capture_output=True,text=True,timeout=3)
        if found.returncode==0 and found.stdout.strip():window=found.stdout.splitlines()[0];break
        time.sleep(.1)
    assert window is not None,'No native reader window'
    flags=Path('/proc/'+str(child.pid)+'/cmdline').read_bytes().split(b'\0')
    assert (b'--deny-exec' in flags)==(not with_profile),flags
    assert all(flag in flags for flag in (b'--deny-net',b'--deny-http',b'--deny-spawn')),flags
    x('windowfocus','--sync',window);time.sleep(.4);click(700,44)
def close():
    key('Escape')
    if child.poll() is None:key('Escape')
    out,err=child.communicate(timeout=10);assert child.returncode==0,(out,err)
def tabs(n):
    for _ in range(n):key('Tab')
def current():return json.loads(profile.read_text())['appearance']
try:
    start();assert not profile.exists(),'Startup must not save a profile'
    for k in ('Return','Tab','Return','Return','Tab','Return','Tab','Return','Return','Tab','Return'):key(k)
    assert not profile.exists(),'Draft must not save'
    tabs(3);key('Return');time.sleep(.5)
    assert profile.exists(),'Save did not publish'
    style=current();assert style['font_family']=='serif' and style['body_size']==20 and style['title_size']==32 and style['font_scale']==1.5 and style['line_height']==1.75,style
    shot('style-saved');saved=profile.read_bytes();close()
    start();shot('style-restarted');assert profile.read_bytes()==saved
    key('Return');tabs(6);key('Return');shot('style-temporary')
    assert profile.read_bytes()==saved,'Apply must not persist';close()
    start();shot('style-after-temporary-restart');assert profile.read_bytes()==saved
    tabs(8);key('Return');time.sleep(.5);reset=current()
    assert reset['font_family']=='sans' and reset['body_size']==16 and reset['title_size']==28 and reset['font_scale']==1 and reset['line_height']==1.45,reset
    shot('style-reset');close()
    start();shot('style-default-restart');assert current()==reset;close()
    backups=list((data/'reader-style').glob('previous-*.json'))
    assert any(p.read_bytes()==saved for p in backups),'Saved profile backup missing'
    profile.write_bytes(b'{invalid');start();shot('style-invalid-fallback')
    assert profile.read_bytes()==b'{invalid','Invalid profile must not be silently rewritten';close()
    start(False);shot('style-default-session-only');assert profile.read_bytes()==b'{invalid';close()
    print(json.dumps(dict(passed=True,default_no_exec_no_profile_writes=True,saved=style,reset=reset,restarts=4,invalid_preserved=True,apply_did_not_write=True,visual_review_required=True)))
finally:
    if child is not None and child.poll() is None:
        child.terminate()
        try:child.communicate(timeout=3)
        except subprocess.TimeoutExpired:child.kill();child.communicate()
'''

def files(root):
 return {p.relative_to(root).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.is_file()}

def main():
 p=argparse.ArgumentParser(description=__doc__);choice=p.add_mutually_exclusive_group(required=True)
 choice.add_argument('--package',type=Path);choice.add_argument('--install-root',type=Path)
 p.add_argument('--launcher',type=Path);p.add_argument('--evidence',type=Path,required=True);a=p.parse_args()
 evidence=a.evidence.absolute();evidence.mkdir(mode=0o700);repo=Path(__file__).resolve().parents[2]
 actual_data=None;launcher=None
 if a.install_root:
  assert a.launcher;root=a.install_root.resolve();launcher=a.launcher.absolute();old_launcher=launcher.read_bytes()
  version=old_launcher.decode().splitlines()[2].removeprefix('# current=');assert '/' not in version and '..' not in version
  package=root/'releases'/version;actual_data=files(root/'data')
 else:package=a.package.resolve()
 before=files(package)
 with tempfile.TemporaryDirectory(prefix='reader-style-native-') as temporary:
  probe=Path(temporary);data=probe/'data';data.mkdir(mode=0o700);(probe/'probe.py').write_text(PROBE)
  sandbox=['bwrap','--unshare-all','--die-with-parent','--ro-bind','/usr','/usr','--ro-bind','/bin','/bin',
    '--ro-bind','/lib','/lib','--ro-bind','/lib64','/lib64','--ro-bind','/etc/fonts','/etc/fonts',
    '--proc','/proc','--dev','/dev','--tmpfs','/tmp','--dir','/tmp/foreign','--bind',str(probe),'/probe',
    '--chdir','/tmp/foreign','--clearenv','--setenv','PATH','/usr/bin:/bin','--setenv','HOME','/tmp','--setenv','LANG','C.UTF-8']
  if a.install_root:
   location=str(root/'data');command=str(launcher)
   sandbox+=['--ro-bind',str(root),str(root),'--bind',str(data),location,'--ro-bind',str(launcher),str(launcher)]
  else:
   location='/data';command='/app/run-reader.sh'
   sandbox+=['--ro-bind',str(package),'/app','--bind',str(data),location,'--setenv','PBLANG_READER_DATA_DIR',location]
  sandbox+=['--setenv','READER_TEST_LAUNCH',command,'--setenv','READER_TEST_DATA',location]
  for system in ('/etc/alternatives','/etc/ImageMagick-6','/etc/ImageMagick-7'):
   if Path(system).is_dir():sandbox+=['--ro-bind',system,system]
  result=subprocess.run(sandbox+['/bin/sh','-c','test ! -e "$1" && exec xvfb-run -a -s "-screen 0 1000x1000x24 -nolisten tcp" env XMODIFIERS=@im=none python3 /probe/probe.py','sh',str(repo)],capture_output=True,text=True,timeout=150)
  (evidence/'native.log').write_text(result.stdout+result.stderr)
  for image in probe.glob('*.png'):shutil.copy2(image,evidence/image.name)
  assert result.returncode==0,(result.stdout,result.stderr)
  result=json.loads(result.stdout);result.update(source_hidden=True,installed_launcher=bool(a.install_root),actual_user_data_untouched=True)
  assert before==files(package)
  if a.install_root:assert old_launcher==launcher.read_bytes() and actual_data==files(root/'data')
  result['package_unchanged']=True
  (evidence/'acceptance.json').write_text(json.dumps(result,indent=2));print(json.dumps(result))
if __name__=='__main__':main()
