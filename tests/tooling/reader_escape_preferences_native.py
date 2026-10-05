"""Source-hidden actual-launcher Escape recovery, explicit Save and held-key journey."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
PREFIX=PREFIX.replace("+(['--with-profile'] if with_profile else [])", "+(['--with-profile'] if with_profile else [])+['--open',str(document)]")
PREFIX=PREFIX.replace(";time.sleep(.4);click(700,44)",";time.sleep(.4)")
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    import hashlib
    from PIL import Image,ImageChops
    document=Path('/probe/Escape — ανάγνωση.html')
    document.write_text('<article><h1>Return safely to reading</h1>'+''.join('<p>Section '+str(i)+'. '+('Readable local words. '*10)+'</p>' for i in range(7))+'<a href="pb://guide">Open guide</a></article>')
    original=document.read_bytes()
    def snapshot():return {p.relative_to(data).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else 'directory' for p in data.rglob('*')}
    def same(a,b,box):
        first=Image.open('/probe/'+a+'.png').convert('RGB');second=Image.open('/probe/'+b+'.png').convert('RGB')
        assert first.size==second.size and ImageChops.difference(first.crop(box),second.crop(box)).getbbox() is None,(a,b)
    start(True);x('windowsize',window,320,480);time.sleep(.5);key('End');shot('escape-before');empty=snapshot();assert not profile.exists()
    click(275,32);tabs(2);shot('escape-style-before');key('Return');shot('escape-draft')
    x('keydown','Escape');time.sleep(1.4);x('keyup','Escape');time.sleep(.3)
    assert child.poll() is None,'Held Escape closed the reader';shot('escape-returned')
    same('escape-before','escape-returned',(0,64,320,480));assert snapshot()==empty
    click(275,32);tabs(2);shot('escape-style-discarded');same('escape-style-before','escape-style-discarded',(0,0,320,448))
    key('Return');key('Next');key('Return');key('Escape');shot('escape-applied');assert child.poll() is None and snapshot()==empty
    click(275,32);tabs(2);shot('escape-style-applied');same('escape-draft','escape-style-applied',(0,0,320,448))
    key('Next');key('Tab');key('Return');time.sleep(.4);assert current()['title_size']==32
    saved=profile.read_bytes();saved_tree=snapshot();key('Escape');shot('escape-saved');assert snapshot()==saved_tree
    click(275,32);tabs(2);key('Return');key('Escape');shot('escape-saved-draft-discarded')
    same('escape-saved','escape-saved-draft-discarded',(0,64,320,480));assert snapshot()==saved_tree
    # Existing Back restoration must survive a settings dismissal at this position.
    key('End');shot('escape-link-position');tabs(2);key('Return');key('Next');shot('escape-guide-position');click(44,32);shot('escape-back')
    same('escape-link-position','escape-back',(0,64,320,480));assert snapshot()==saved_tree
    click(120,32);shot('escape-forward');same('escape-guide-position','escape-forward',(0,64,320,480))
    # Close while Style is open: first dismissal, then deliberate document close.
    click(275,32);close();assert document.read_bytes()==original and profile.read_bytes()==saved
    start(True);shot('escape-restarted');assert current()['title_size']==32 and snapshot()==saved_tree
    click(700,44);key('Escape');assert child.poll() is None;close()
    assert document.read_bytes()==original and snapshot()==saved_tree
    print(json.dumps(dict(passed=True,real_launcher=True,held_escape_kept_window=True,unapplied_discarded=True,applied_kept=True,saved_kept=True,save_explicit=True,no_extra_profile_writes=True,back_anchor_kept=True,forward_anchor_kept=True,restart_saved_style=True,input_unchanged=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
