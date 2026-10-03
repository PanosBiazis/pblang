# Archive mode comparison

Run each file with the same PBLang0.6.0 runtime, from the repository root:
`pblang run bench/secure_archive/binary_fast.sapp --allow-all`.
Use the matching `auto` file as the existing-mode baseline. Each process packs
four identical768-byte payloads; the final printed number is ZIP byte length.
The repeat workload is64 repeats of `abc def ghi `; binary is a fixed synthetic
768-byte array shared byte-for-byte across modes. No user data or hardware effects.

Recorded protocol: one warmup and seven measured processes per mode/workload,
alternating mode order; CPU(user+system), peak process RSS/working set and wall
clock. Linux used `/usr/bin/time`; Windows used System.Diagnostics.Process and
Stopwatch, with an explicit working directory. Raw samples are in
`docs/benchmarks/secure_archive_{linux,windows}_2026-10-03.json`.
With seven samples, nearest-rank p95 equals the largest observed sample, not a
reliable tail bound. Both systems were ordinary active desktops, not controlled
performance laboratories. The retained runtimes share version0.6.0 but are
different platform builds. Compare modes within each system; these observations
do not rank the computers or establish energy, GPU/NPU, disk, network or global
RAM savings. Builds and fixture generation are outside the measured intervals;
process startup and parsing are included. CRC/security verification remains in
functional tests; this benchmark measures packing only.

Windows working-set reads after process exit returned zero and are **unavailable**:
`rss_kib:null`, with the observed zero retained separately in `rss_raw_kib`.
They are not evidence of zero RAM use. Windows CPU measurements are quantized;
the repeat workload reported zero CPU ticks, which cannot establish its true
CPU cost. Its wall-time observations remain usable with desktop-noise limits.

The same archive suite and28 measured samples also ran on a second authorized
Linux x86_64 laptop. Its raw observations are in
`secure_archive_second_linux_2026-10-03.json` in the same benchmark documentation
directory. Only the synthetic acceptance package was transferred; its temporary
remote directory was removed afterward. No corporate server was used.
