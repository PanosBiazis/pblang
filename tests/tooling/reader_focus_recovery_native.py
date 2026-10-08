"""Real launcher: recover a link after an unavailable large-text layout; respect new focus."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    def same(a,b):
        x=Image.open('/probe/'+a+'.png').convert('RGB');y=Image.open('/probe/'+b+'.png').convert('RGB')
        assert x.size==y.size and ImageChops.difference(x,y).getbbox() is None,(a,b)
    start();app=Path('/proc/'+str(child.pid)+'/cwd').resolve()
    source='let S=import('+json.dumps(str(app/'examples/declarative_browser/appearance_store.sapp'))+');let p='+json.dumps(str(data))+';let old=S.load(p);let a=old.appearance;a.title_size=40;a.font_scale=1.5;let r=S.save(p,a,old.token);assert(r.ok and r.owned_handles_closed,"large-text profile saved");'
    probe=Path('/probe/profile.sapp');probe.write_text(source)
    result=subprocess.run([str(app/'pblang'),'run',str(probe),'--allow-all','--deny-net','--deny-http','--deny-spawn'],cwd=app,capture_output=True,text=True,timeout=20);assert result.returncode==0,(result.stdout,result.stderr)
    close();saved=profile.read_bytes()
    start();key('Escape');key('Tab');key('Return');shot('guide-reference');close()
    start();key('Escape');key('Tab');shot('wide-focused-link');x('windowsize',window,320,240);time.sleep(.5);shot('unavailable-layout')
    x('windowsize',window,800,816);time.sleep(.5);shot('restored-link');key('Return');shot('guide-after-recovery');same('guide-reference','guide-after-recovery');close()
    start();key('Escape');x('keydown','Shift_L');key('Tab');x('keyup','Shift_L');time.sleep(.2);shot('home-focus-reference');key('Tab');key('Tab')
    x('windowsize',window,320,240);time.sleep(.5);key('Tab');x('windowsize',window,800,816);time.sleep(.5);shot('new-focus-retained');same('home-focus-reference','new-focus-retained');close()
    assert profile.read_bytes()==saved,'Focus/layout must not write appearance'
    print(json.dumps(dict(passed=True,guide_pixels_match_reference=True,new_input_cancels_recovery=True,profile_unchanged=True,sizes=[[800,816],[320,240]],visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
