"""Real launcher: precise font sizes, compact navigation, explicit Save and restart."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    start();x('windowsize',window,320,240);time.sleep(.5);key('Page_Down')
    key('Left');key('Right');key('Right');shot('fine-compact')
    assert not list(data.rglob('*')),'Fine adjustment implicitly saved'
    x('windowsize',window,800,816);time.sleep(.5);key('Tab');key('Left');key('Right');key('Right');shot('fine-wide')
    tabs(5);key('Return');shot('fine-saved')
    assert current()['body_size']==17 and current()['title_size']==29,current()
    saved=profile.read_bytes();key('Escape');shot('fine-document');assert child.poll() is None;close()
    start();shot('fine-restarted');assert profile.read_bytes()==saved and current()['body_size']==17 and current()['title_size']==29
    key('Tab');key('Left');key('Escape');assert profile.read_bytes()==saved,'Discard changed saved profile';close()
    print(json.dumps(dict(passed=True,body_size=17,title_size=29,no_implicit_save=True,explicit_save=True,restart_kept=True,escape_discard_kept_saved=True,compact_size=[320,240],wide_size=[800,816],visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
