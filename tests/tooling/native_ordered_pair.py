"""Check same-title input isolation on a fresh Xvfb display; synthetic text only."""
import argparse,json,os,shlex,signal,subprocess,tempfile,time
from pathlib import Path
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--runtime',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);a=p.parse_args()
 display=os.environ.get('DISPLAY','').split('.')[0];servers=subprocess.check_output(['ps','-C','Xvfb','-o','args='],text=True)
 assert display and any(display in shlex.split(line)[1:] for line in servers.splitlines()),'isolated Xvfb required'
 a.evidence.mkdir(mode=0o700);repo=Path(__file__).resolve().parents[2]
 with tempfile.TemporaryDirectory(prefix='pb-input-pair-') as tmp:
  saved=Path(tmp)/'values.json';env=dict(os.environ,LANG='C.UTF-8',XMODIFIERS='@im=none',PBLANG_INPUT_DIAG_OUT=str(saved))
  child=subprocess.Popen([str(a.runtime.resolve()),'run','tests/fixtures/native_ordered_pair.sapp','--allow-all','--deny-exec','--deny-net','--deny-http','--deny-spawn'],cwd=repo,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
  def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=3).strip()
  try:
   end=time.monotonic()+8;windows=[]
   while time.monotonic()<end:
    if child.poll() is not None:raise AssertionError(child.communicate())
    found=subprocess.run(['xdotool','search','--name','^PBLang Input Pair$'],capture_output=True,text=True,timeout=2)
    windows=found.stdout.splitlines()
    if len(windows)==2:break
    time.sleep(.05)
   assert len(windows)==2
   x('windowfocus','--sync',windows[0]);time.sleep(.2);x('type','--delay','0','abcdef')
   subprocess.run(['setxkbmap','gr'],check=True,timeout=3)
   mapping=subprocess.check_output(['xmodmap','-pke'],text=True,timeout=3)
   assert all(k in mapping.split() for k in ('Greek_alpha','Greek_beta','Greek_gamma'))
   x('windowfocus','--sync',windows[1]);time.sleep(.2);x('key','--delay','1','Greek_alpha','Greek_beta','Greek_gamma');time.sleep(.3)
   x('windowraise',windows[1]);subprocess.run(['import','-window',windows[1],str(a.evidence/'greek-field.png')],check=True,timeout=5)
   out,err=child.communicate(timeout=8);assert child.returncode==0,(out,err)
   values=json.loads(out.strip().splitlines()[-1]);assert sorted(values)==sorted(['abcdef','αβγ']),values
   assert json.loads(saved.read_text())==values
   result={'passed':True,'same_public_title':True,'separate_values':values,'exact_save_readback':True,'visual_review_required':True}
   (a.evidence/'summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False))
  finally:
   if child.poll() is None:
    os.killpg(child.pid,signal.SIGTERM)
    try:child.communicate(timeout=2)
    except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.communicate()
if __name__=='__main__':main()
