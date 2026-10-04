"""Real keyboard/resize journey on a task-owned Xvfb only; no personal desktop."""
import argparse, json, os, shlex, signal, subprocess, time
from pathlib import Path

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime',type=Path);parser.add_argument('--fixture',type=Path);parser.add_argument('--work-dir',type=Path);parser.add_argument('--cwd',type=Path)
    args=parser.parse_args()
    display=os.environ.get('DISPLAY','').split('.')[0]
    servers=subprocess.check_output(['ps','-C','Xvfb','-o','args='],text=True)
    if not display or not any(display in shlex.split(line)[1:] for line in servers.splitlines()):
        raise SystemExit('Use an isolated xvfb-run display.')
    root=args.work_dir or Path(__file__).resolve().parents[2];state_file=root/'build/reader-measured.json'
    state_file.parent.mkdir(exist_ok=True);state_file.unlink(missing_ok=True)
    env=dict(os.environ)
    for key in ('PBLANG_GRAPHICS_IDLE_PUMP_MS','PBLANG_GRAPHICS_MAX_MS','PBLANG_GRAPHICS_MAX_FRAMES'):env.pop(key,None)
    child=subprocess.Popen([str(args.runtime or root/'target/release/pblang'),'run',str(args.fixture or root/'tests/fixtures/reader_text_fit_native.sapp'),'--allow-all','--deny-net','--deny-http','--deny-exec','--deny-spawn'],cwd=args.cwd or root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
    observations=[]
    def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=6).strip()
    def settled(**expected):
        deadline=time.monotonic()+10;state=None
        while time.monotonic()<deadline:
            try:
                state=json.loads(state_file.read_text())
                if all(state.get(k)==v for k,v in expected.items()):
                    observations.append(state);return state
            except (FileNotFoundError,json.JSONDecodeError):pass
            if child.poll() is not None:raise AssertionError(('probe stopped',child.communicate()))
            time.sleep(.08)
        raise AssertionError(('state timeout',expected,state))
    def press(key,hold=.18):x('keydown',key);time.sleep(hold);x('keyup',key);time.sleep(.2)
    def shot(name):subprocess.run(['import','-window',window,str(root/'build'/name)],check=True,timeout=8)
    try:
        deadline=time.monotonic()+15;window=None
        while time.monotonic()<deadline:
            if child.poll() is not None:raise AssertionError(('probe stopped before window',child.communicate()))
            result=subprocess.run(['xdotool','search','--name','^PBLang measured reader$'],capture_output=True,text=True,timeout=2)
            if result.returncode==0 and result.stdout.strip():window=result.stdout.splitlines()[0];break
            time.sleep(.1)
        assert window is not None,'probe window did not appear'
        x('windowfocus','--sync',window)
        first=settled(width=320,height=240,available=False,bounds=True,exact=True)
        assert first['baseline_clipped_lines']>0
        assert first['help'].replace(' ','')=='Enlarge window or change Style.'.replace(' ','')
        assert first['footer'].replace(' ','')=='Local links only • No network access'.replace(' ','')
        shot('reader-measured-help.png')
        x('windowsize',window,320,480);first=settled(width=320,height=480,available=True,page=0,bounds=True,exact=True);shot('reader-measured-first.png')
        for page in range(1,int(first['pages'])):
            press('Next');settled(page=page,bounds=True,exact=True)
        last=settled(page=first['pages']-1,bounds=True,exact=True);anchor=last['anchor'];shot('reader-measured-last.png')
        x('windowsize',window,640,720);settled(width=640,height=720,anchor=anchor,bounds=True,exact=True);shot('reader-measured-wide.png')
        x('windowsize',window,320,240);small=settled(width=320,height=240,available=False,anchor=anchor,bounds=True)
        assert small['help'].replace(' ','')=='Enlarge window or change Style.'.replace(' ','')
        x('windowsize',window,320,480);settled(width=320,height=480,available=True,anchor=anchor,bounds=True,exact=True)
        press('End');end=json.loads(state_file.read_text());settled(page=end['pages']-1,bounds=True,exact=True)
        for _ in range(9):
            state=json.loads(state_file.read_text())
            if state['focus']=='block1':break
            press('Tab')
        settled(focus='block1');press('Return');settled(url='pb://about',bounds=True,exact=True)
        press('Escape');output,error=child.communicate(timeout=15)
        assert child.returncode==0 and 'reader_text_fit_native_pass' in output,(output,error)
        print(json.dumps({'result':'pass','observations':observations,'port_or_hardware_access':False},ensure_ascii=False))
    finally:
        if child.poll() is None:
            os.killpg(child.pid,signal.SIGTERM)
            try:child.communicate(timeout=3)
            except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.communicate()
if __name__=='__main__':main()
