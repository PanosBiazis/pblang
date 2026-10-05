"""Actual source-hidden launcher: explicit keyboard recovery after profile conflicts."""
import reader_appearance_native as harness
import reader_reload_native as reload_harness
PREFIX=reload_harness.PREFIX
CLEANUP=reload_harness.CLEANUP
HELPERS=reload_harness.JOURNEY.split('\ntry:\n',1)[0]
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    def same(a,b):
        x=Image.open('/probe/'+a+'.png').convert('RGB');y=Image.open('/probe/'+b+'.png').convert('RGB')
        assert x.size==y.size and ImageChops.difference(x,y).getbbox() is None,(a,b)
    def different(a,b):
        x=Image.open('/probe/'+a+'.png').convert('RGB');y=Image.open('/probe/'+b+'.png').convert('RGB')
        assert x.size==y.size and ImageChops.difference(x,y).getbbox() is not None,(a,b)
    start();pid=child.pid;key('Return');tabs(7);key('Return');assert current()['font_family']=='serif'
    changed=external('monospace');choose();choose();tabs(7)
    before=stored_files();x('keydown','Return');time.sleep(1.4);x('keyup','Return');time.sleep(.3);shot('recovery-wide-conflict')
    assert profile.read_bytes()==changed and stored_files()==before,'Conflict/held key wrote profile'
    key('Return');shot('recovery-wide-keyboard');different('recovery-wide-conflict','recovery-wide-keyboard');assert stored_files()==before,'Keyboard Reload wrote profile'
    reload();shot('recovery-wide-reference');same('recovery-wide-keyboard','recovery-wide-reference')
    choose();tabs(7);key('Return');assert current()['font_family']=='system'
    shot('recovery-wide-saved')
    # Capacity-one settings: failed Save must reveal and focus Reload on another page.
    x('windowsize',window,320,240);time.sleep(.4);key('Home');key('Return')
    changed=external('serif');key('End');key('Prior');key('Prior')
    before=stored_files();x('keydown','Return');time.sleep(1.4);x('keyup','Return');time.sleep(.3);shot('recovery-compact-conflict')
    assert stored_files()==before and profile.read_bytes()==changed
    key('Return');shot('recovery-compact-keyboard');different('recovery-compact-conflict','recovery-compact-keyboard');assert stored_files()==before
    key('End');key('Return');shot('recovery-compact-reference');same('recovery-compact-keyboard','recovery-compact-reference')
    key('Home');key('Return');key('End');key('Prior');key('Prior');key('Return')
    assert current()['font_family']=='monospace';shot('recovery-compact-saved')
    assert child.pid==pid and child.poll() is None;key('Escape');shot('recovery-returned');assert child.poll() is None
    saved=profile.read_bytes();close();start();assert profile.read_bytes()==saved;shot('recovery-restarted');close()
    print(json.dumps(dict(passed=True,real_launcher=True,same_gui_pid=True,external_writer='PBLang Store.save',wide_keyboard_recovery=True,compact_keyboard_recovery=True,held_enter_no_recovery=True,reload_read_only=True,explicit_save_after_reload=True,escape_preserved=True,restart_preserved=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+HELPERS+JOURNEY+CLEANUP
    harness.main()
