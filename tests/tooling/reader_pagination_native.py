"""Real keyboard/resize journey on a task-owned Xvfb only; no personal desktop."""
import json, os, shlex, signal, subprocess, time
from pathlib import Path

def main():
    display=os.environ.get('DISPLAY','').split('.')[0]
    servers=subprocess.check_output(['ps','-C','Xvfb','-o','args='],text=True)
    if not display or not any(display in shlex.split(line)[1:] for line in servers.splitlines()):
        raise SystemExit('Use an isolated xvfb-run display.')
    root=Path(__file__).resolve().parents[2];state_file=root/'build/reader-pagination-native.json'
    state_file.parent.mkdir(exist_ok=True);state_file.unlink(missing_ok=True)
    env=dict(os.environ)
    for key in ('PBLANG_GRAPHICS_IDLE_PUMP_MS','PBLANG_GRAPHICS_MAX_MS','PBLANG_GRAPHICS_MAX_FRAMES'):env.pop(key,None)
    child=subprocess.Popen([str(root/'target/release/pblang'),'run','tests/fixtures/reader_pagination_native.sapp','--allow-all'],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
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
        window=x('search','--sync','--name','^PBLang paginated reader$').splitlines()[0];x('windowfocus','--sync',window)
        first=settled(page=0,settings=False,bounds=True);assert first['pages']>1;shot('reader-pages-large.png')
        press('Next',.7);second=settled(page=1,bounds=True);anchor=second['anchor'];shot('reader-pages-next.png')
        for _ in range(3):press('Tab')
        settled(focus='appearance');press('Return');settled(settings=True,focus='font_family')
        press('Tab');settled(focus='body_size');press('Return');settled(body_size=14,focus='body_size')
        x('keydown','Shift_L');time.sleep(.2);press('Tab');press('Tab');x('keyup','Shift_L');time.sleep(.2)
        settled(focus='appearance');press('Return');settled(settings=False,anchor=anchor,bounds=True)
        x('windowsize',window,320,480);settled(width=320,height=480,anchor=anchor,bounds=True);shot('reader-pages-narrow.png')
        press('End');end=json.loads(state_file.read_text());settled(page=end['pages']-1,bounds=True);shot('reader-pages-last.png')
        for _ in range(8):
            state=json.loads(state_file.read_text())
            if state['focus']=='block3':break
            press('Tab')
        settled(focus='block3');press('Return');settled(url='pb://about');shot('reader-pages-destination.png')
        press('Escape');output,error=child.communicate(timeout=15)
        assert child.returncode==0 and 'reader_pagination_native_pass' in output,(output,error)
        print(json.dumps({'result':'pass','observations':observations,'port_or_hardware_access':False},ensure_ascii=False))
    finally:
        if child.poll() is None:
            os.killpg(child.pid,signal.SIGTERM)
            try:child.communicate(timeout=3)
            except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.communicate()
if __name__=='__main__':main()
