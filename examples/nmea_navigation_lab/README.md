# NMEA Navigation Lab

This PBLang-only example extracts and substantially hardens the useful GPS
protocol behavior from an earlier Python networking and Bluetooth experiment.
The shared `stdlib:nmea.sapp` library provides bounded NMEA-0183 framing,
checksum generation and verification, GGA/RMC parsing, coordinate conversion,
calendar validation, batch limits and privacy-safe views.

The original live paths disclosed public IP, Bluetooth addresses and exact
location, depended on external services, contained an always-true exit branch
and used a placeholder GATT characteristic. They are not reproduced. Live BLE
and public-IP geolocation remain explicitly fail-closed until reviewed native
PBLang permission, discovery and characteristic-selection adapters exist.

A shared responsive UI tree compiles to Web, native desktop and Android records.
The default command output contains coarse coordinates and excludes raw NMEA
sentences and device identifiers.

```text
pblang run examples/nmea_navigation_lab/main.sapp
pblang run examples/nmea_navigation_lab/benchmark.sapp
```

The benchmark exercises the complete 128-sentence public batch bound, verifies
every parse and builds a privacy projection for every record. It is a bounded
parser workload, not evidence for live receiver throughput or safety-critical
navigation performance.

This is a protocol and educational navigation parser, not a certified safety,
aviation, maritime or emergency-navigation system.
