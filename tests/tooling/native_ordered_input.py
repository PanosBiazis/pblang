"""Exact native keyboard acceptance on an isolated Xvfb display, synthetic data only."""
import argparse,json,os,shlex,signal,subprocess,tempfile,time
from pathlib import Path
CASES={
 'latin-slow':('us',list('abc'),.12,'abc'),
 'latin-fast':('us',list('abcdef'),.001,'abcdef'),
 'overlap':('us',['b','a'],.12,'ba'),
 'greek-slow':('gr',['Greek_alpha','Greek_beta','Greek_gamma'],.12,'αβγ'),
 'greek-fast':('gr',['Greek_alpha','Greek_beta','Greek_gamma'],.001,'αβγ'),
 'greek-medium':('gr',['Greek_alpha','Greek_beta','Greek_gamma'],.02,'αβγ'),
 'burst':('us',list('abcdefghijklmnopqrstuvwxyz0123456789'),0,'abcdefghijklmnopqrstuvwxyz0123456789'),
 'shortcut':('us',list('abc'),.04,'x'),
 'keypad':('us',['KP_1','KP_2','KP_0','KP_Decimal'],.02,'120.'),
}
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--runtime',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);p.add_argument('--mode',choices=['ordered','legacy'],default='ordered');p.add_argument('--case',choices=CASES,action='append',required=True);a=p.parse_args()
 display=os.environ.get('DISPLAY','').split('.')[0];servers=subprocess.check_output(['ps','-C','Xvfb','-o','args='],text=True)
 assert display and any(display in shlex.split(line)[1:] for line in servers.splitlines()),'isolated Xvfb required'
 a.evidence.mkdir(mode=0o700);repo=Path(__file__).resolve().parents[2];results=[]
 def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=5).strip()
 for name in a.case:
  layout,keys,hold,expected=CASES[name]
  with tempfile.TemporaryDirectory(prefix='pb-ordered-input-') as tmp:
   saved=Path(tmp)/'synthetic.txt';env=dict(os.environ,PBLANG_INPUT_DIAG_OUT=str(saved),PBLANG_INPUT_ORDERED='1' if a.mode=='ordered' else '0',LANG='C.UTF-8',XMODIFIERS='@im=none')
   for k in ('PBLANG_GRAPHICS_IDLE_PUMP_MS','PBLANG_GRAPHICS_MAX_MS','PBLANG_GRAPHICS_MAX_FRAMES'):env.pop(k,None)
   child=subprocess.Popen([str(a.runtime.resolve()),'run','tests/fixtures/native_keyboard_boundary.sapp','--allow-all','--deny-exec','--deny-net','--deny-http','--deny-spawn'],cwd=repo,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
   try:
    deadline=time.monotonic()+8;window=None
    while time.monotonic()<deadline:
     if child.poll() is not None:
      output,error=child.communicate();(a.evidence/(name+'-startup.json')).write_text(json.dumps({'returncode':child.returncode,'stdout':output,'stderr':error},indent=2));raise AssertionError((output,error))
     found=subprocess.run(['xdotool','search','--name','^PBLang Input Boundary$'],capture_output=True,text=True,timeout=2)
     if found.returncode==0 and found.stdout.strip():window=found.stdout.splitlines()[0];break
     time.sleep(.05)
    assert window is not None,'No input fixture window'
    # Xvfb resets its keymap when its last client disconnects. Configure only
    # after the owned window connects, and prove the requested keysyms exist.
    before=subprocess.check_output(['xmodmap','-pke'],text=True,timeout=5)
    subprocess.run(['setxkbmap',layout],check=True,timeout=5)
    keymap=subprocess.check_output(['xmodmap','-pke'],text=True,timeout=5)
    if layout=='gr':assert all(key in keymap.split() for key in keys),'stable Greek keymap required; do not synthesize temporary mappings'
    (a.evidence/(name+'-keymap.json')).write_text(json.dumps({'layout':layout,'before_has_keys':all(key in before.split() for key in keys),'after_has_keys':all(key in keymap.split() for key in keys),'selected_rows':[line for line in keymap.splitlines() if any(key in line.split() for key in keys)]},ensure_ascii=False,indent=2))
    x('windowfocus','--sync',window);time.sleep(.25)
    if name=='keypad':x('key','Num_Lock');time.sleep(.1)
    if name=='overlap':x('keydown',*keys);time.sleep(hold);x('keyup',*keys)
    elif name=='burst':x('type','--delay','0',expected)
    else:
     for key in keys:x('keydown',key);time.sleep(hold);x('keyup',key);time.sleep(hold)
    if name=='shortcut':x('key','ctrl+a');x('key','x')
    if name=='keypad':x('key','Num_Lock')
    output,error=child.communicate(timeout=7);assert child.returncode==0,(output,error)
    value=json.loads(output.strip().splitlines()[-1]);value.update(case=name,mode=a.mode,layout=layout,expected=expected,exact=value['value']==expected,save_reopen_exact=saved.read_text()==value['value']==value['reopened'],hold_ms=hold*1000)
    (a.evidence/(name+'.json')).write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n');results.append({k:value[k] for k in ('case','mode','input_backend','expected','value','exact','save_reopen_exact','hold_ms')})
   finally:
    if child.poll() is None:
     os.killpg(child.pid,signal.SIGTERM)
     try:child.communicate(timeout=2)
     except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.communicate()
 (a.evidence/'summary.json').write_text(json.dumps(results,ensure_ascii=False,indent=2)+'\n');print(json.dumps(results,ensure_ascii=False))
 assert all(r['save_reopen_exact'] for r in results)
 if a.mode=='ordered':assert all(r['exact'] and r['input_backend']=='x11-ordered-keysyms' for r in results),'native input acceptance failed'
if __name__=='__main__':main()
