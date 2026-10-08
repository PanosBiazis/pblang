"""Actual launcher: session zoom, explicit Save, restart, and temporary reset."""
import reader_appearance_native as harness
PREFIX=harness.PROBE.split('\ntry:\n',1)[0]
CLEANUP='\nfinally:'+harness.PROBE.rsplit('\nfinally:',1)[1]
JOURNEY=r'''
try:
    from PIL import Image,ImageChops
    def chord(name):
        x('keydown','Control_L');time.sleep(.18);key(name);x('keyup','Control_L');time.sleep(.22)
    def same(a,b):
        left=Image.open('/probe/'+a+'.png').convert('RGB');right=Image.open('/probe/'+b+'.png').convert('RGB')
        return left.size==right.size and ImageChops.difference(left,right).getbbox() is None
    start();key('Escape');shot('default-reading');assert not profile.exists()
    chord('equal');shot('zoomed-reading');assert not same('default-reading','zoomed-reading'),'Zoom must change actual reading pixels'
    assert not profile.exists(),'Zoom must not save';key('Return');shot('zoomed-style');tabs(7);key('Return')
    assert current()['font_scale']==1.05,current();saved=profile.read_bytes();shot('zoom-saved');close()
    start();key('Escape');shot('zoom-reopened');assert same('zoomed-reading','zoom-reopened'),'Saved scale must reopen identically';assert profile.read_bytes()==saved
    chord('minus');shot('zoom-decreased');assert same('default-reading','zoom-decreased'),'Ctrl+Minus restores original size'
    chord('equal');chord('0');shot('zoom-reset');assert same('default-reading','zoom-reset'),'Ctrl+0 resets only session scale';assert profile.read_bytes()==saved;close()
    start();assert current()['font_scale']==1.05 and profile.read_bytes()==saved;key('Escape');shot('saved-after-temporary-reset');assert same('zoomed-reading','saved-after-temporary-reset');close()
    print(json.dumps(dict(passed=True,session_zoom_no_save=True,explicit_save_scale=1.05,reopen_pixels_match=True,minus_and_reset_pixels_match=True,temporary_reset_preserved_saved_profile=True,visual_review_required=True)))
'''
if __name__=='__main__':
    harness.PROBE=PREFIX+JOURNEY+CLEANUP
    harness.main()
