# Solar Daylight Planner

A PBLang-only migration of an earlier Python astronomy plotting exercise. The
original source depended on Ephem, NumPy, Matplotlib and Tk, contained empty
observer coordinates, assumed one unnamed timezone, and did not handle polar
day or polar night.

## Improvements

- validated Gregorian dates, latitude, longitude and explicit UTC offset;
- bounded daily or sampled annual series for years 1600..9999;
- sunrise, apparent solar noon, sunset and daylight duration;
- explicit tagged polar-day and polar-night results without invented times;
- event times include local clock and adjacent-day offset;
- deterministic local 2D line and 3D scatter plans without external assets;
- one responsive and accessible UI tree for Web, native and Android records;
- coordinates remain in memory and no network request is made;
- malformed date, coordinate, timezone and work-budget regression tests.

The calculation is a fractional-year solar approximation using a conventional
90.833-degree apparent zenith. It is suitable for educational planning, not
navigation, legal, safety-critical or observatory-grade ephemeris work.
Moonrise and moonset are intentionally not approximated: a separately verified
lunar ephemeris is required before those capabilities can be advertised.

## Run

```sh
pblang run examples/solar_daylight_planner/main.sapp
```

## Verify

```sh
pblang run tests/semantics/astronomy_test.sapp
pblang run examples/solar_daylight_planner/test.sapp --allow-all
pblang run examples/solar_daylight_planner/benchmark.sapp
```
