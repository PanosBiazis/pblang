"""Real launcher: compact paging across an unavailable Save without writes."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    def same(a,b):
        x=Image.open('/probe/'+a+'.png').convert('RGB');y=Image.open('/probe/'+b+'.png').convert('RGB')
        assert x.size==y.size and ImageChops.difference(x,y).getbbox() is None,(a,b)
    profile.parent.mkdir(parents=True);profile.write_bytes(b'{invalid');before={str(p):p.read_bytes() for p in data.rglob('*') if p.is_file()}
    start();pid=child.pid;x('windowsize',window,320,240);time.sleep(.5);key('Home')
    for _ in range(6):key('Next')
    shot('paging-apply');key('Next');shot('paging-forward')
    key('End');key('Prior');shot('paging-reset-reference');same('paging-forward','paging-reset-reference')
    key('Prior');shot('paging-backward');same('paging-backward','paging-apply')
    # Mouse pager and held keyboard pager use the same bounded destination rule.
    click(230,136);shot('paging-pointer');same('paging-pointer','paging-reset-reference')
    key('Prior');x('keydown','Next');time.sleep(1.3);x('keyup','Next');time.sleep(.3)
    shot('paging-held');same('paging-held','paging-reset-reference')
    key('Return');shot('paging-reset-applied');key('Escape');shot('paging-document')
    assert child.pid==pid and child.poll() is None;close()
    assert {str(p):p.read_bytes() for p in data.rglob('*') if p.is_file()}==before,'Read-only paging/reset changed storage'
    print(json.dumps(dict(passed=True,disabled_save_skipped_both_directions=True,mouse_pager=True,held_key_one_move=True,reset_reachable=True,invalid_profile_unchanged=True,same_gui_pid=True,compact_size=[320,240],visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
