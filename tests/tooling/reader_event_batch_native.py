"""Actual launcher: resize and Tab queued together retain the correct focus order."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    import signal
    from PIL import Image,ImageChops
    def same(a,b):
        first=Image.open('/probe/'+a+'.png').convert('RGB');second=Image.open('/probe/'+b+'.png').convert('RGB')
        assert first.size==second.size and ImageChops.difference(first,second).getbbox() is None,(a,b)
    start();x('windowsize',window,320,240);time.sleep(.5);shot('batch-small')
    # Pause only this isolated test child; queue the real X11 resize/key events.
    os.kill(child.pid,signal.SIGSTOP)
    try:
        x('windowsize',window,800,816);time.sleep(.15);x('keydown','Tab')
    finally:os.kill(child.pid,signal.SIGCONT)
    time.sleep(.25);x('keyup','Tab');time.sleep(.25);key('Return');shot('batch-resize-edit')
    assert not list(data.rglob('*')),'Batched edit implicitly saved'
    close()
    # Independent real UI reference: resize settles before Tab/Enter.
    start();x('windowsize',window,320,240);time.sleep(.4);x('windowsize',window,800,816);time.sleep(.5)
    key('Tab');key('Return');shot('batch-sequential-reference');same('batch-resize-edit','batch-sequential-reference')
    tabs(6);key('Return');assert current()['body_size']==18 and current()['font_family']=='sans'
    saved=profile.read_bytes();shot('batch-saved');key('Escape');shot('batch-returned');assert child.poll() is None;close()
    start();shot('batch-restarted');assert profile.read_bytes()==saved and current()['body_size']==18;close()
    print(json.dumps(dict(passed=True,real_launcher=True,queued_resize_tab_matches_sequential=True,body_size_saved=18,font_family_unchanged=True,no_implicit_save=True,escape_kept=True,restart_kept=True,only_isolated_test_child_paused=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
