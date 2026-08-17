# Verification

Verification date: 2026-08-14

## PBLang checks

- `test.sapp`: passed. It validates the UI tree, compact and tablet layouts,
  generated JVM class bytes, manifest security policy, button behavior, and
  bounded PBLang security analysis of the production sources.
- `emulator_test.sapp`: 64/64 API/architecture cases passed in 1.86 seconds
  with 32,588 KiB peak host RSS. The matrix covers API 21 through 36 on ARMv7,
  ARM64, x86, and x86_64 profiles.
- PBLang lint: passed for all four `.sapp` files.
- Self-hosted PBLang parser/type checks: passed for all four `.sapp` files.
- Package manifest validation: passed with one relative `stdlib` dependency.

The PBLang emulator is a userspace/runtime conformance model. It does not claim
kernel, ART, Binder, GPU-driver, radio, or vendor-framework emulation.

## APK artifact

- Path: `release/AndroidDevExample-debug.apk`
- SHA-256: `b25af6b7d36adcfd236790e8f8d68d0cf13143aa1b6451073990f359ad2494ea`
- Size: 12,715 bytes
- Package: `com.example.myapp`
- Activity: `com.example.myapp.MainActivity`
- SDK contract: minimum API 21, target API 34, compiled with API 36
- Source implementation: PBLang; Android activity implementation:
  PBLang-generated JVM bytecode
- Signature verification: APK signature schemes v1, v2, and v3 passed
- ZIP alignment: passed
- Permissions: none requested
- Security manifest policy: backup disabled, cleartext disabled, large heap
  disabled, hardware acceleration enabled

## Physical Android 9 verification

Device: TCL/Alcatel 3X `5048Y_EEA`, Android 9, API 28, ARM64.

- APK transfer and Package Manager installation: passed.
- First measured launch: 617 ms.
- Three subsequent force-stop/cold-launch checks: 445, 455, and 453 ms.
- The real button tap changed the displayed text to
  `Welcome to Android Programming!!!`.
- The application rendered at 720x1600 with no overlap or clipped text.
- Crash/ANR log scan: no matching event.
- Post-launch PSS: 39,023 KiB, including 13,564 KiB attributed to Android system
  sharing and 6,311 KiB graphics memory.

The device's streamed `adb install` path stalled without output. A bounded
`adb push` followed by `pm install -r -t` succeeded. This is a device/vendor ADB
installation-path issue, not an application crash.

## Compatibility boundary

The verified support contract is Android 5.0/API 21 and newer. Android 1.x-4.x
remain outside the current modern UI backend and were not represented by real
system images or physical devices in this verification. API-level emulator
conformance is useful evidence, but it is not a substitute for physical and
official-system-image testing on every vendor release.
