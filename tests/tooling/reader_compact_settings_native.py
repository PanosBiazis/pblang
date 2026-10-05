"""Keyboard-only preferences at the minimum viewport; owns an isolated Xvfb child."""
import json, os, shlex, signal, subprocess, time
from pathlib import Path

def main():
    display=os.environ.get('DISPLAY','').split('.')[0]
    servers=subprocess.check_output(['ps','-C','Xvfb','-o','args='],text=True)
    if not display or not any(display in shlex.split(line)[1:] for line in servers.splitlines()):
        raise SystemExit('Use an isolated xvfb-run display.')
    root=Path(__file__).resolve().parents[2];state_file=root/'build/reader-compact-settings.json'
    state_file.parent.mkdir(exist_ok=True);state_file.unlink(missing_ok=True)
    env=dict(os.environ)
    for key in ('PBLANG_GRAPHICS_IDLE_PUMP_MS','PBLANG_GRAPHICS_MAX_MS','PBLANG_GRAPHICS_MAX_FRAMES'):env.pop(key,None)
    child=subprocess.Popen([str(root/'target/release/pblang'),'run','tests/fixtures/reader_compact_settings_native.sapp','--allow-all'],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
    observations=[]
    def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=6).strip()
    def settled(**expected):
        deadline=time.monotonic()+10;state=None
        while time.monotonic()<deadline:
            try:
                state=json.loads(state_file.read_text())
                if all(state.get(k)==v for k,v in expected.items()):observations.append(state);return state
            except (FileNotFoundError,json.JSONDecodeError):pass
            if child.poll() is not None:raise AssertionError(('probe stopped',child.communicate()))
            time.sleep(.08)
        raise AssertionError(('state timeout',expected,state))
    def press(key,hold=.18):x('keydown',key);time.sleep(hold);x('keyup',key);time.sleep(.2)
    def shot(name):subprocess.run(['import','-window',window,str(root/'build'/name)],check=True,timeout=8)
    try:
        window=x('search','--sync','--name','^PBLang compact preferences$').splitlines()[0];x('windowfocus','--sync',window)
        initial=settled(settings=False,bounds=True)
        for _ in range(3):press('Tab')
        settled(focus='appearance');press('Return');settled(settings=True,focus='font_family',bounds=True);shot('reader-settings-small.png')
        for name,value in [('font_family','serif'),('body_size',18),('title_size',32),('font_scale',1.25),('line_height',1.75),('density','compact')]:
            settled(focus=name,bounds=True);press('Return');state=settled(focus=name,bounds=True);assert state['appearance'][name]==value
            if name!='density':press('Next',.7)
        shot('reader-settings-last.png');x('windowsize',window,390,844);settled(width=390,height=844,focus='density',layout=None,bounds=True);shot('reader-settings-expanded.png')
        x('windowsize',window,320,240);last=settled(width=320,height=240,focus='density',bounds=True);assert last['layout']['page']==5
        press('Home');settled(focus='font_family',bounds=True);press('End');settled(focus='density',bounds=True)
        x('keydown','Shift_L');press('Tab');x('keyup','Shift_L');settled(focus='settings_previous')
        press('space',.7);state=settled(focus='line_height',bounds=True);assert state['appearance']['line_height']==1.75
        press('End');settled(focus='density');x('keydown','Shift_L');press('Tab');press('Tab');x('keyup','Shift_L')
        settled(focus='appearance');press('Return');settled(settings=False,focus='appearance',anchor=initial['anchor'],bounds=True);shot('reader-settings-done.png')
        press('Escape');press('Escape');output,error=child.communicate(timeout=15)
        assert child.returncode==0 and 'reader_compact_settings_native_pass' in output,(output,error)
        print(json.dumps({'result':'pass','observations':observations,'device_access':False},ensure_ascii=False))
    finally:
        if child.poll() is None:
            os.killpg(child.pid,signal.SIGTERM)
            try:child.communicate(timeout=3)
            except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.communicate()
if __name__=='__main__':main()
