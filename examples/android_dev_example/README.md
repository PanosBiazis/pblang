# Android Development Example

This is the PBLang-only conversion of `project1`. The application keeps the
original interaction: pressing the button changes the message to
`Welcome to Android Programming!!!`.

The source contains no Java, Kotlin, XML layout, Gradle project, or AndroidX
dependency. PBLang compiles the declarative UI to JVM class bytes and uses only
the Android SDK format tools to create, align, and sign the APK.

## Files

- `app.sapp`: shared application UI, constants, and Android build options.
- `main.sapp`: APK build entry point with verified signing and metadata.
- `test.sapp`: UI behavior, Android bytecode, responsive-layout, and manifest
  policy checks.
- `emulator_test.sapp`: Android API 21-36 and ARMv7/ARM64/x86/x86_64
  userspace/runtime conformance matrix.
- `release/AndroidDevExample-debug.apk`: installable debug APK after a build.

## Build and verify

Run these commands from the `products/pblang` directory:

```sh
pblang run examples/android_dev_example/test.sapp --allow-all
pblang run examples/android_dev_example/emulator_test.sapp --allow-all
pblang run examples/android_dev_example/main.sapp --allow-all
```

Install on a connected Android device:

```sh
adb install -r examples/android_dev_example/release/AndroidDevExample-debug.apk
```

The generated application supports Android API 21 and newer, adapts to phone
and tablet widths, allows orientation changes, uses 48 dp minimum touch targets,
disables cleartext traffic and backup, and requests no permissions. The included
PBLang emulator test covers every integer API level from 21 through 36 across
four CPU architecture profiles. It is a bounded userspace/runtime conformance
model, not a Linux-kernel, ART, GPU-driver, radio, or vendor-firmware emulator.

Android 1.x-4.x are outside the current modern UI backend because its real
widget/theme contract starts at API 21. Supporting those releases requires a
separate legacy renderer and physical or system-image verification; this example
does not claim that compatibility.
