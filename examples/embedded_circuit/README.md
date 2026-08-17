# Portable Embedded Circuit

This PBLang-only example controls two board instances through one portable
state model. The UNO LED blinks every 500 ms for three seconds while the ESP32
LED remains on. It runs without connected hardware in deterministic emulator
mode:

```sh
pblang run examples/embedded_circuit/main.sapp --allow-all
```

Use `mode:"wired"` only with a validated `/dev/ttyUSB*` mapping. Wired state is
reported as pending until a real transport acknowledges the observed state; it
is never silently treated as successful. Serial identities are not stored in
the circuit snapshot.

The state model is the shared backend for the planned desktop, web, and Android
LED controller. It deliberately contains no renderer-specific logic.
