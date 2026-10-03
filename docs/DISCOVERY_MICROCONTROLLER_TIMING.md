# Relative microcontroller sample timing

`stdlib:sample_clock.sapp` converts caller-supplied wrapping counter values to relative elapsed ticks. It does not open a serial port, read a clock/GPS, reset or flash hardware, or change WiFi/Bluetooth/hotspot settings.

`timeline(ticks, bits, max_step)` accepts 1..256 strict integer counters, 8..32 counter bits, and an integral forward-step bound in 1..(2^(bits-1)-1). Every counter must be in 0..(2^bits-1). Adjacent differences are computed modulo 2^bits and must be in 1..max_step. Duplicates, half-cycle ambiguity, and larger steps are rejected. Results contain relative elapsed ticks starting at zero, deltas, total ticks, and inferred wraps; the input array is preserved. Maximum accumulated count is 255*(2^31-1) = 547608329985 ticks, exactly representable below 2^53.

This algorithm **assumes** a forward-running counter and at most the declared interval between adjacent observations. It cannot detect resets or whole missing wrap cycles that happen to produce plausible counters. Reordering within the bound may also remain indistinguishable. Do not infer missing samples, device identity, authenticity, elapsed wall time, or a successful hardware capture from acceptance. `inferred_wraps` counts numerical decreases accepted under that assumption.

`period_review(ticks, bits, max_step, expected_step)` uses the same timeline validation. Expected period is a strict integer in 1..max_step. It reports signed errors (observed minus expected), maximum absolute error, MAE and RMS error in ticks. With one sample there are no intervals: statistics are `nil`, not zero. Squared-error accumulation uses floating-point arithmetic; unlike accumulated integer ticks, its large values are approximate. Tick frequency/unit conversion, clock drift, sensor accuracy, timestamp synchronization and scheduling remain the caller's responsibility.

The example combines this library with existing `sample_series.sapp`. Synthetic 32-bit counters `[4294967280,4,24,44]` become `[0,20,40,60]`; interpolation at relative ticks 10,30,50 produces 1.5,2.5,3.5. Another sequence has interval errors `[0,1,-1]`. No physical board or phone is needed.

Run `pblang run tests/semantics/sample_clock_test.sapp` and `pblang run examples/discovery_microcontroller_timing.sapp` from a locked checkout.

Existing related capabilities: `nmea.sapp` and `examples/nmea_navigation_lab` already provide bounded GPS GGA/RMC framing/parsing/privacy views; `embedded_targets.sapp` describes boards; `embedded_protocol.sapp` implements a separate command sequence/checksum contract. Counter timing does not replace that protocol. In the inspected source the UNO R4 WiFi descriptor is planned, its pin map is empty and the RA4M1 backend is incomplete. Historical ATmega328P UNO or classic ESP32 controller tests do not establish support for the user's currently connected boards. Alcatel GPS/WiFi/Bluetooth/hotspot access is not implemented or tested by this example.

Errors contain constant messages without caller values. Runtime diagnostics can nevertheless show source lines containing literals; this library does not redact the runtime. Relative timing and derived measurements may still be sensitive. It has no logging/persistence or external dependencies.
