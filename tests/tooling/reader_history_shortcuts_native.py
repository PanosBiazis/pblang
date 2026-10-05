"""Real source-hidden launcher: Alt+arrows, held keys and preferences isolation."""
import reader_appearance_native as harness
import reader_history_position_native as history
PREFIX=history.PREFIX
CLEANUP=history.CLEANUP
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    document=Path('/probe/Keyboard — ανάγνωση.html')
    document.write_text('<article><h1>Keep reading with the keyboard</h1>'+''.join('<p>Section '+str(i)+'. '+('Readable local words. '*10)+'</p>' for i in range(7))+'<a href="pb://guide">Open guide</a></article>')
    original=document.read_bytes()
    def same(a,b,box=(0,64,320,480)):
        first=Image.open('/probe/'+a+'.png').convert('RGB');second=Image.open('/probe/'+b+'.png').convert('RGB')
        assert first.size==second.size and ImageChops.difference(first.crop(box),second.crop(box)).getbbox() is None,(a,b)
    def chord(arrow,hold=.18,extra=None):
        if extra:x('keydown',extra)
        x('keydown','Alt_L');time.sleep(.1);x('keydown',arrow);time.sleep(hold);x('keyup',arrow);x('keyup','Alt_L')
        if extra:x('keyup',extra)
        time.sleep(.3)
    start(True);x('windowsize',window,320,480);time.sleep(.5);key('End');shot('keys-home-last')
    tabs(4);key('Return');key('End');shot('keys-guide-last')
    chord('Left');shot('keys-back');same('keys-home-last','keys-back')
    chord('Right');shot('keys-forward');same('keys-guide-last','keys-forward')
    key('Left');key('Right');chord('Left',extra='Control_L');shot('keys-unmodified');same('keys-forward','keys-unmodified')
    # A third history entry makes unintended auto-repeat observable.
    click(200,32);key('End');chord('Left',hold=1.4);shot('keys-held-back');same('keys-guide-last','keys-held-back')
    chord('Left');shot('keys-second-back');same('keys-home-last','keys-second-back')
    chord('Right');chord('Right');shot('keys-second-forward');same('keys-home-last','keys-second-forward')
    # A settings draft must survive both chords with its focused choice unchanged.
    click(275,32);tabs(2);key('Return');shot('keys-style-draft')
    chord('Left');chord('Right');shot('keys-style-kept');same('keys-style-draft','keys-style-kept',(0,0,320,480))
    key('Escape');shot('keys-style-dismissed');same('keys-home-last','keys-style-dismissed')
    chord('Left');shot('keys-after-dismissal');same('keys-guide-last','keys-after-dismissal')
    assert child.poll() is None and not list(data.rglob('*')) and document.read_bytes()==original
    close()
    print(json.dumps(dict(passed=True,real_launcher=True,back_restored=True,forward_restored=True,held_key_single_step=True,separate_press_moves=True,plain_arrows_unchanged=True,ctrl_alt_ignored=True,preferences_draft_focus_kept=True,escape_kept=True,no_profile_or_position_writes=True,input_unchanged=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
