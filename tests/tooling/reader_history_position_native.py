"""Real source-hidden reader: Back/Forward positions, reflow, Home and fresh launch."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
PREFIX=PREFIX.replace("+(['--with-profile'] if with_profile else [])", "+(['--with-profile'] if with_profile else [])+['--open',str(document)]")
PREFIX=PREFIX.replace(";time.sleep(.4);click(700,44)",";time.sleep(.4)")
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    document=Path('/probe/Remember — θέση.html')
    source='<article><h1>Remember my reading place</h1>'+''.join('<p>Section '+str(i)+'. '+('Readable local words. '*10)+'</p>' for i in range(7))+'<a href="pb://guide">Open the guide</a></article>'
    document.write_text(source);original=document.read_bytes()
    def same_body(a,b):
        first=Image.open('/probe/'+a+'.png').convert('RGB');second=Image.open('/probe/'+b+'.png').convert('RGB')
        assert first.size==second.size
        # History button availability/focus changes; compare the complete reading area.
        assert ImageChops.difference(first.crop((0,64,*first.size)),second.crop((0,64,*second.size))).getbbox() is None,(a,b)
    start(False);x('windowsize',window,320,480);time.sleep(.5);shot('history-first')
    key('End');shot('history-home-last');tabs(4);key('Return');shot('history-guide-first')
    key('Next');shot('history-guide-position')
    click(44,32);shot('history-back');same_body('history-home-last','history-back')
    click(120,32);shot('history-forward');same_body('history-guide-position','history-forward')
    # Reflow while away: increase body size, Apply, then return to the saved source anchor.
    click(275,32);key('Tab');key('Return');key('Next');key('Return');click(275,32)
    click(44,32);shot('history-back-reflow')
    click(200,32);shot('history-home-reset')
    assert not list(data.rglob('*')) and document.read_bytes()==original
    close()
    # Independent UI reference: reflow the original position without navigating away.
    # The logical anchor need not remain on the last numbered page after reflow.
    start(False);x('windowsize',window,320,480);time.sleep(.5);key('End')
    click(275,32);key('Tab');key('Return');key('Next');key('Return');click(275,32)
    shot('history-reflow-reference');same_body('history-back-reflow','history-reflow-reference')
    click(200,32);shot('history-home-reference');same_body('history-home-reset','history-home-reference')
    close();start(False);x('windowsize',window,320,480);time.sleep(.5);shot('history-fresh-launch')
    same_body('history-first','history-fresh-launch');close()
    assert not list(data.rglob('*')) and document.read_bytes()==original
    print(json.dumps(dict(passed=True,real_launcher=True,back_restored=True,forward_restored=True,reflow_return_restored=True,new_launch_starts_first=True,no_position_or_profile_files=True,input_unchanged=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
