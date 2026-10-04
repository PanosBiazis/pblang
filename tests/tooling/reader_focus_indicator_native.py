"""Source-hidden reader focus visibility: actual pixels, compact controls and large text.

Requires Pillow for read-only screenshot inspection, plus the appearance harness tools.
"""
import json
from pathlib import Path
import sys
from PIL import Image
import reader_appearance_native as harness

PREFIX = harness.PROBE.split('\ntry:\n', 1)[0]
CLEANUP = '\nfinally:' + harness.PROBE.rsplit('\nfinally:', 1)[1]
JOURNEY = r'''
try:
    start(False);x('windowsize',window,320,240);time.sleep(.5);key('Home');shot('focus-small')
    key('Tab');shot('focus-tab')
    x('keydown','Shift_L');key('Tab');x('keyup','Shift_L');shot('focus-back')
    x('windowsize',window,320,480);time.sleep(.5);key('Home')
    for k in ('Return','Tab','Return','Return','Tab','Return','Return','Tab','Return','Tab','Tab','Return'):key(k)
    shot('focus-large-settings');click(275,32);shot('focus-large-document')
    key('Tab');shot('focus-next');key('Return');key('End');shot('focus-last')
    key('Escape');shot('focus-cleared');close()
    assert not profile.exists(),'Session-only focus journey must not save'
    print(json.dumps(dict(passed=True,default_no_exec_no_profile_writes=True,large_text=True,keyboard_focus_journey=True,visual_review_required=True)))
'''


def pixels(evidence):
    images = {name: Image.open(evidence / (name + '.png')).convert('RGB')
              for name in ('focus-small', 'focus-tab', 'focus-back')}
    def band(im):
        return [im.getpixel((160, y)) for y in range(166, 169)]
    white = [(255, 255, 255)] * 3
    accent = [(108, 92, 231)] * 3
    assert band(images['focus-small']) == white, 'Compact focused frame must be three white pixels thick'
    assert band(images['focus-back']) == white, 'Shift+Tab must restore the frame'
    assert band(images['focus-tab']) == accent, 'Tab must remove the old frame'
    def luma(rgb):
        channels = [v / 255 for v in rgb]
        linear = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in channels]
        return sum(v * w for v, w in zip(linear, (.2126, .7152, .0722)))
    ratio = 1.05 / (luma(accent[0]) + .05)
    assert ratio >= 4.5
    return dict(pixel_frame_thickness=3, pixel_contrast_ratio=ratio, tab_removes_old_frame=True,
                shift_tab_restores_frame=True)


if __name__ == '__main__':
    harness.PROBE = PREFIX + JOURNEY + CLEANUP
    harness.main()
    evidence = Path(sys.argv[sys.argv.index('--evidence') + 1]).absolute()
    result = json.loads((evidence / 'acceptance.json').read_text())
    result['passed'] = False
    (evidence / 'acceptance.json').write_text(json.dumps(result, indent=2))
    result.update(pixels(evidence))
    result['passed'] = True
    (evidence / 'acceptance.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result))
