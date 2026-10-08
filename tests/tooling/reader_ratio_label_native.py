"""Real launcher: long ratio labels fit while canonical precise values survive Save/restart."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    start();app=Path('/proc/'+str(child.pid)+'/cwd').resolve()
    def run(source):
        path=Path('/probe/ratio.sapp');path.write_text(source)
        result=subprocess.run([str(app/'pblang'),'run',str(path),'--allow-all','--deny-net','--deny-http','--deny-spawn'],cwd=app,capture_output=True,text=True,timeout=20)
        assert result.returncode==0,(result.stdout,result.stderr)
        return result.stdout
    def module(name):return json.dumps(str(app/'examples/declarative_browser'/name))
    run('let S=import('+module('appearance_store.sapp')+');let p='+json.dumps(str(data))+';let old=S.load(p);let a=old.appearance;a.font_scale=0.9999999999999999;a.line_height=1.9999999999999998;let r=S.save(p,a,old.token);assert(r.ok and r.owned_handles_closed,"canonical precise profile saved");')
    close();start();saved=profile.read_bytes();precise=current();shot('wide-ratios')
    x('windowsize',window,320,240);time.sleep(.5);key('Home')
    for _ in range(3):key('Next')
    shot('compact-scale');key('Next');shot('compact-spacing')
    assert profile.read_bytes()==saved,'Viewing must not rewrite precise values'
    for _ in range(3):key('Next')
    key('Return');shot('compact-saved');assert current()==precise,'Save rounded precise values';close();start();assert current()==precise;shot('restarted-ratios');close()
    metrics=run('let A=import('+module('app.sapp')+');let GUI=import('+json.dumps(str(app/'stdlib/gui.sapp'))+');let G=import('+json.dumps(str(app/'stdlib/graphics.sapp'))+');let c=A.create();A.configure(c,map{font_scale:0.9999999999999999,line_height:1.9999999999999998});c.width=320;c.height=240;c.settings=true;let win=G.Window(320,240,"Ratio metrics");let rows=[];for style in [map{font_scale:0.9999999999999999,line_height:1.9999999999999998},map{font_scale:1.23456789012345,line_height:1.45678901234567}] {A.configure(c,style);for name in ["font_scale","line_height"] {c.focus_target=name;A.refresh(c);let w=GUI.get(c.form,name);let m=win.textMeasure(w.text,w.font_size,0,w.font_family);push(rows,map{name:name,text:w.text,width:m.width,available:w.w-24,ok:m.ok});}}win.close();c.session.close();print json_stringify(rows);')
    measured=json.loads(metrics);assert all(r['ok'] and r['width']<=r['available'] for r in measured),measured
    print(json.dumps(dict(passed=True,precise_values=precise,viewing_did_not_write=True,save_restart_preserved=True,native_label_metrics=measured,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
