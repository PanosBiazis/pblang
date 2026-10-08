"""Real launcher: Home discards drafts like Escape; reading pixels and storage stay intact."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    def same(a,b):
        x=Image.open('/probe/'+a+'.png').convert('RGB');y=Image.open('/probe/'+b+'.png').convert('RGB')
        assert x.size==y.size and ImageChops.difference(x,y).getbbox() is None,(a,b)
    start();pid=child.pid
    for label,width,height,home_x,style_x in [('wide',800,816,475,700),('compact',320,240,195,275)]:
        x('windowsize',window,width,height);time.sleep(.5);key('Escape');click(3,height-3);shot(label+'-document-before')
        click(style_x,32);key('Return');click(home_x,32);click(3,height-3);shot(label+'-document-after')
        same(label+'-document-before',label+'-document-after')
        click(style_x,32);shot(label+'-navigation-discard')
        key('Return');key('Escape');click(style_x,32);shot(label+'-escape-reference')
        same(label+'-navigation-discard',label+'-escape-reference')
    assert child.pid==pid and child.poll() is None;assert not list(data.rglob('*')),'Navigation wrote preferences';close()
    print(json.dumps(dict(passed=True,home_matches_escape=True,reading_pixels_unchanged=True,no_implicit_save=True,same_gui_pid=True,sizes=[[800,816],[320,240]],visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
