"""Real launcher: a keyboard-focused link remains usable after shrinking to 320x240."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    def same(a,b):
        x=Image.open('/probe/'+a+'.png').convert('RGB');y=Image.open('/probe/'+b+'.png').convert('RGB')
        return x.size==y.size and ImageChops.difference(x,y).getbbox() is None
    # Reference: activate the guide while its wide button is already visible.
    start(False);key('Escape');key('Tab');key('Return');x('windowsize',window,320,240);time.sleep(.5);shot('guide-reference');close()
    start(False);key('Escape');key('Tab');shot('wide-focused-link')
    x('windowsize',window,320,240);time.sleep(.5);shot('compact-focused-link')
    key('Return');shot('guide-after-resize');close()
    assert same('guide-reference','guide-after-resize'),'Focused-link Enter after resize did not navigate to the reference guide'
    assert not list(data.rglob('*')),'Resize/navigation wrote preferences'
    print(json.dumps(dict(passed=True,keyboard_link_survives_resize=True,guide_pixels_match_reference=True,no_profile_writes=True,sizes=[[800,816],[320,240]],visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
