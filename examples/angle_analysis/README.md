# Angle Analysis

This native PBLang example evolves a small interactive degree-conversion
exercise into a reusable and validated angle model. It uses the public
`deg_to_rad` and `rad_to_deg` functions from `stdlib:math.sapp` and contains no
foreign source or runtime fallback.

The earlier exercise accepted only an integer, calculated sine and cosine, and
printed each result twice before and after rounding. This edition accepts
bounded decimal degrees and reports:

- the original and normalized angle;
- radians, sine, cosine, and tangent;
- an explicit undefined tangent on the vertical axes;
- axis or quadrant location;
- reference angle and unit-vector coordinates;
- configurable display precision without changing the underlying result.

## Run

From the PBLang repository:

```sh
./target/release/pblang run examples/angle_analysis/main.sapp -- 855 6
./target/release/pblang run examples/angle_analysis/main.sapp -- -90
```

## Boundaries

Input is decimal text limited to 64 characters and an absolute value of
1,000,000,000,000 degrees. Exponent notation and non-finite textual values are
rejected. Display precision is limited to 0 through 12 decimal places.

This example performs numerical analysis only. It does not claim symbolic
algebra, arbitrary-precision trigonometry, graphical plotting, or physical
measurement support.
