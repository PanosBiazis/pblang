"""Diagnose retained native input loss on an isolated Xvfb display; no product data."""
import argparse,json,os,shlex,signal,subprocess,tempfile,time
from pathlib import Path

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--runtime',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);a=p.parse_args()
    display=os.environ.get('DISPLAY','').split('.')[0]
    servers=subprocess.check_output(['ps','-C','Xvfb','-o','args='],text=True)
    assert display and any(display in shlex.split(line)[1:] for line in servers.splitlines()),'isolated Xvfb required'
    a.evidence.mkdir(mode=0o700);repo=Path(__file__).resolve().parents[2];results=[]
    def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=5).strip()
    for name,keys,hold,expected in [('slow',['a','b','c'],.12,'abc'),('rapid',list('abcdef'),.001,'abcdef'),('overlap',['b','a'],.12,'ba'),('greek',['U03b1','U03b2'],.18,'αβ')]:
        with tempfile.TemporaryDirectory(prefix='pb-input-boundary-') as tmp:
            saved=Path(tmp)/'synthetic.txt';env=dict(os.environ,PBLANG_INPUT_DIAG_OUT=str(saved))
            for k in ('PBLANG_GRAPHICS_IDLE_PUMP_MS','PBLANG_GRAPHICS_MAX_MS','PBLANG_GRAPHICS_MAX_FRAMES'):env.pop(k,None)
            child=subprocess.Popen([str(a.runtime.resolve()),'run','tests/fixtures/native_keyboard_boundary.sapp','--allow-all','--deny-exec','--deny-net','--deny-http','--deny-spawn'],cwd=repo,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            try:
                window=x('search','--sync','--name','^PBLang Input Boundary$').splitlines()[0];x('windowfocus','--sync',window);time.sleep(.35)
                if name=='overlap':
                    x('keydown',*keys);time.sleep(hold);x('keyup',*keys)
                else:
                    for key in keys:x('keydown',key);time.sleep(hold);x('keyup',key);time.sleep(hold)
                output,error=child.communicate(timeout=6);assert child.returncode==0,(output,error)
                value=json.loads(output.strip().splitlines()[-1]);assert saved.read_text()==value['value']==value['reopened']
                value.update(case=name,injected=keys,expected=expected,exact=value['value']==expected,save_reopen_exact=True,hold_ms=hold*1000)
                (a.evidence/(name+'.json')).write_text(json.dumps(value,ensure_ascii=False,indent=2))
                results.append({k:value[k] for k in ('case','expected','value','exact','save_reopen_exact','hold_ms')})
            finally:
                if child.poll() is None:
                    os.killpg(child.pid,signal.SIGTERM)
                    try:child.communicate(timeout=2)
                    except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.communicate()
    (a.evidence/'summary.json').write_text(json.dumps(results,ensure_ascii=False,indent=2));print(json.dumps(results,ensure_ascii=False))

if __name__=='__main__':main()
