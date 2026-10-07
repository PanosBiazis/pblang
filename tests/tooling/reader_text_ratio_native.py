"""Real Reader launcher: fine scale/spacing in compact UI, Save, restart and discard."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    start();tabs(3)
    for k in ('Left','Right','Right','Tab','Left','Right','Right'):key(k)
    shot('ratio-wide');assert not list(data.rglob('*')),'Ratio editing implicitly saved'
    x('windowsize',window,320,240);time.sleep(.5);key('Left');key('Right');shot('ratio-compact')
    key('End');key('Page_Up');key('Page_Up');key('Return');shot('ratio-saved')
    assert current()['font_scale']==1.05 and current()['line_height']==1.5,current()
    saved=profile.read_bytes();key('Escape');shot('ratio-document');assert child.poll() is None;close()
    start();shot('ratio-restarted');assert profile.read_bytes()==saved and current()['font_scale']==1.05 and current()['line_height']==1.5
    tabs(3);key('Left');key('Escape');assert profile.read_bytes()==saved,'Discard changed saved ratios';close()
    print(json.dumps(dict(passed=True,font_scale=1.05,line_height=1.5,no_implicit_save=True,explicit_save=True,restart_kept=True,escape_discard_kept_saved=True,compact_size=[320,240],wide_size=[800,816],visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
