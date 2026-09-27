# Offline sensor replay lessons

Run `pblang run examples/discovery_sensor_replay.sapp` after generating the
package lock with `pblang --lock .`. All input is synthetic and embedded in
the example. There is no device, network, timer, model or file access.

This delivery reuses `sample_series.sapp`; it does not introduce another
interpolator or claim a physical sensor integration.

Four executable cases are included:

1. Uneven timestamps `[0,1000,3000,6000]` and values `[10,14,18,12]` replayed
   through piecewise linear interpolation. The estimate at 2000 ms is 16;
   the one at 4500 ms is 15. Estimates are labeled as estimates.
2. Complete trailing two-sample windows produce `[12,16,15]`, aligned with
   timestamps `[1000,3000,6000]`. This is a sample mean, not a time-weighted
   average: long intervals do not receive more weight.
3. A caller-selected maximum gap of 2000 ms rejects the estimate at 4500 ms
   while retaining actual measured endpoints, including 6000 ms. This gap
   limit is illustrative, not a physical sensor specification. If no queries
   remain, callers should report that explicitly instead of invoking the
   existing interpolator with an empty query array.
4. Out-of-range queries, repeated timestamps and reversed timestamps throw.
   Rejection is asserted; no synthetic success value stands in for failure.

The source arrays remain unchanged. Existing sample-series limits apply:
paired interpolation arrays with 2–256 samples, 1–4096 queries, strictly
increasing positions and finite values within ±1e9. Moving means accept
1–4096 samples and an integer window within the sample count.

Inspiration came from locally retained sensor and audio-bridge documentation
and the existing discovery catalogue. No external implementation was copied.
This is an educational replay example, not realtime acquisition, packet-loss
recovery, clock synchronization, device calibration or a safety controller.
