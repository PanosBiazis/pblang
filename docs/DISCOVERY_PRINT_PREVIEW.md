# Physical-unit print preview lessons

Run `pblang run examples/discovery_print_preview.sapp` after `pblang --lock .`.
The example reuses `units.in_to_cm` and `rect_clip.canvas`, with a recording
sink instead of a printer or graphics window. All dimensions are synthetic.

Four executable cases cover:

1. A 210×297 mm sheet with 10 mm margins at 254 DPI becomes 2100×2970
   pixels, with a 1900×2770 pixel printable region starting at (100,100).
   This DPI is selected for simple arithmetic, not as a printer recommendation.
2. Four repeated rectangles crossing page margins produce three clipped
   rectangles; the fourth is completely outside and generates no sink call.
3. Fractional pixel margins round inward: left/top use ceiling and right/bottom
   use floor. The sample's 10×10 mm page yields a 96×94 pixel interior at (2,3).
4. Empty physical areas, nonnumeric DPI and areas without a whole printable
   pixel are rejected. No fallback silently draws outside the declared area.

The example helper accepts dimensions from 1 to 2000 mm, integer DPI from
36 through 1200, and exactly four nonnegative finite margins ordered left,
top, right, bottom. This is a local teaching helper, not a new stdlib API.
Coordinates use the existing half-open pixel clipping contract.

The chosen margins are caller-supplied. Real hardware has its own printable
area, driver scaling, paper tolerances and calibration. No printer was queried,
no print job was submitted, and no PDF, raster image or driver was generated.
Results are geometric evidence only, not proof of physical print accuracy.

The implementation is original synthetic code inspired by the existing
pattern-print-workflow proposal and general printable-area concepts. It
contains no company document data, device configuration or third-party driver
code. Scaling a real pattern and checking it with a physical calibration mark
remain separate integration steps.
