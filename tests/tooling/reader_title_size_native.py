"""Real launcher title customization across imported documents; synthetic data only."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
PREFIX=PREFIX.replace("+(['--with-profile'] if with_profile else [])", "+(['--with-profile'] if with_profile else [])+['--open',str(document)]")
PREFIX=PREFIX.replace(";time.sleep(.4);click(700,44)",";time.sleep(.4)")
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    document=Path('/probe/Title — imported.html')
    document.write_text('<article><h1>Readable title — Τίτλος</h1><p>'+('Reading text. '*18)+'</p><a href="pb://guide">Guide</a></article>')
    original=document.read_bytes()
    start();shot('title-default-document');click(700,44);tabs(2);shot('title-default-control')
    key('Return');shot('title-draft-32');assert not profile.exists()
    tabs(5);key('Return');time.sleep(.4);saved=profile.read_bytes();style=current()
    assert style['title_size']==32 and style['body_size']==16 and style['font_scale']==1,style
    click(700,44);shot('title-saved-document');close();assert document.read_bytes()==original
    # A second --open uses the exact same saved v1 style; neither input is changed.
    second=Path('/probe/Second document.html');second.write_text('<article><h1>Second document</h1><p>A new article retains the saved reading appearance.</p></article>')
    second_original=second.read_bytes();document=second
    start();shot('title-second-document');assert profile.read_bytes()==saved
    click(700,44);x('windowsize',window,320,480);time.sleep(.6);tabs(2);shot('title-compact-saved')
    key('Return');shot('title-compact-draft-36');assert profile.read_bytes()==saved
    key('Next');shot('title-compact-actions');key('Return');assert profile.read_bytes()==saved
    click(275,32);shot('title-session-36');close();assert second.read_bytes()==second_original
    start();shot('title-restart-32');assert profile.read_bytes()==saved
    click(700,44);tabs(8);key('Return');time.sleep(.4);reset=current()
    assert reset['title_size']==28 and reset['body_size']==16 and reset['font_scale']==1,reset
    shot('title-reset-control');click(700,44);shot('title-reset-document');close()
    assert any(p.read_bytes()==saved for p in (data/'reader-style').glob('previous-*.json'))
    start();shot('title-reset-restart');assert current()==reset;close()
    assert second.read_bytes()==second_original
    print(json.dumps(dict(passed=True,real_launcher=True,saved_title=32,reset_title=28,new_document_same_saved_profile=True,apply_did_not_write=True,inputs_unchanged=True,backup_preserved=True,compact_choices_and_actions=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
