"""Source-hidden explicit reload after an external PBLang writer changes the profile.

Reuse the appearance harness's package/installed isolation and synthetic data.
Review screenshots; machine checks alone do not assert the displayed typography.
"""
import reader_appearance_native as harness

PREFIX = harness.PROBE.split('\ntry:\n', 1)[0]
CLEANUP = '\nfinally:' + harness.PROBE.rsplit('\nfinally:', 1)[1]
JOURNEY = r'''
def external(family):
    app=Path('/proc/'+str(child.pid)+'/cwd').resolve()
    source='let Store=import('+json.dumps(str(app/'examples/declarative_browser/appearance_store.sapp'))+');'
    source+='let parent='+json.dumps(str(data))+';let loaded=Store.load(parent);let style=loaded.appearance;'
    source+='style.font_family='+json.dumps(family)+';let result=Store.save(parent,style,loaded.token);assert(result.ok,"External writer failed");print json_stringify(result);'
    Path('/probe/external.sapp').write_text(source)
    p=subprocess.run([str(app/'pblang'),'run','/probe/external.sapp','--allow-all','--deny-net','--deny-http','--deny-spawn'],cwd=app,capture_output=True,text=True,timeout=20)
    assert p.returncode==0,(p.stdout,p.stderr)
    result=json.loads(p.stdout);assert result['ok'] and result['owned_handles_closed']
    return profile.read_bytes()
def choose():click(400,182)
def save():click(400,546)
def reload():click(400,650)
def stored_files():return {p.name:p.read_bytes() for p in profile.parent.iterdir() if p.is_file()}
try:
    start();pid=child.pid;choose();save();assert current()['font_family']=='serif'
    original=profile.read_bytes();changed=external('monospace')
    choose();choose();save();shot('reload-conflict')
    assert profile.read_bytes()==changed,'Stale GUI overwrote external profile'
    before=stored_files();reload();shot('reload-restored');assert stored_files()==before,'Reload wrote files'
    choose();save();assert current()['font_family']=='system','Reload did not renew the revision/draft'
    shot('reload-saved');valid=profile.read_bytes();choose()
    for name,invalid in [('invalid',b'{broken'),('utf8',b'\xff'),('oversize',b'x'*4097)]:
        profile.write_bytes(invalid);before=stored_files()
        for _ in range(3):reload()
        save();assert stored_files()==before,'Failed reload or disabled Save changed files'
        shot('reload-'+name)
    profile.unlink();before=stored_files();reload();save();assert stored_files()==before and not profile.exists()
    shot('reload-missing')
    # Restore synthetic external bytes, then create repeated genuine store revisions.
    profile.write_bytes(valid);profile.chmod(0o600)
    for family in ('serif','monospace','system'):
        external(family);before=stored_files();reload();assert stored_files()==before
        external('sans');changed=profile.read_bytes();save();assert profile.read_bytes()==changed
        reload();choose();save();assert current()['font_family']=='serif'
    assert child.pid==pid and child.poll() is None,'Recovery restarted the GUI'
    shot('reload-churn-recovered');saved=profile.read_bytes()
    x('windowsize',window,320,240);time.sleep(.5);key('End');shot('reload-compact');key('Return')
    assert profile.read_bytes()==saved,'Compact reload wrote files'
    close()
    start();shot('reload-restart');assert profile.read_bytes()==saved;close()
    start(False);shot('reload-default-session');assert profile.read_bytes()==saved;close()
    print(json.dumps(dict(passed=True,same_gui_pid=True,external_writer='PBLang Store.save',conflict_reload_save=True,
        reload_read_only=True,invalid_utf8_oversize_missing_preserved=True,repeated_invalid_reads=9,
        churn_rounds=3,default_no_exec_no_profile_writes=True,visual_review_required=True)))
'''

if __name__ == '__main__':
    harness.PROBE = PREFIX + JOURNEY + CLEANUP
    harness.main()
