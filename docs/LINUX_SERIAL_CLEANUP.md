# Linux serial restoration result

`stdlib:linux_serial.sapp` keeps the existing `close(state)` result fields:
`ok` and `termios_restored`. Both describe the combined result of restoration
attempts required by the state. When modem lines were captured, a failed
modem-line restoration must remain a failure even if the subsequent termios
restoration succeeds. The historical field name `termios_restored` is retained
for compatibility with embedded controller and ESP32 callers.

Both restorations are attempted, in order, before releasing the descriptor,
termios buffers and library handles. A restoration failure does not skip the
remaining cleanup. The state is then marked closed and its descriptor and
buffer fields invalidated. A repeated close performs no more effects and
retains its existing successful no-op result; retain the **first** close result
when reporting restoration evidence.

This result is not a general proof that every resource release succeeded.
Descriptor-close and library-close errors are not aggregated by the existing
API, and native crashes or hostile mutation of the state are not covered.
No retry of an ambiguously closed descriptor is introduced.

## Synthetic regression check

Run from the checkout, supplying the retained PBLang executable:

```sh
python3 tests/tooling/linux_serial_close_faults.py --runtime /path/to/pblang
```

The test copies the owning module into a temporary directory and substitutes
only the two FFI primitive names with deterministic PBLang fixtures. It runs
with native FFI, process execution, networking, environment access and GUI
access disabled. It never opens a serial device or PTY. The production close
body, internal error conversion, restoration ordering and state updates are
executed unchanged.

Sixteen cases cover success, error return and thrown primitive error for both
restorations; configuration-dependent restoration; absent descriptor or
termios snapshot; repeated close; and the existing nil-state no-op. They also
assert that the expected remaining release calls occur exactly once.

This synthetic check validates control flow and reported results, not physical
modem signals, real driver restoration or cross-platform serial support.
