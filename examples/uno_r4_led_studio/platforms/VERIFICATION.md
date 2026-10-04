# Platform acceptance — 2026-10-04

This records bounded acceptance, not universal end-to-end completion.

| Surface | Delivered | Observed | Remaining |
| --- | --- | --- | --- |
| Android | PBLang-generated debug APK, own package identity | Installed on authorized USB Android device; Activity foreground; actual editor screenshot; form navigation | Needs paired Linux server/ADB USB; no independent PBLang Android VM, direct Arduino USB driver, or APK download picker |
| Linux native | Shared PBLang view with explicit-runtime launcher | Existing live native application; source launcher argument/exit tests | Physical controls in this slice are separate Web/CLI host operations |
| Windows x86-64 | Runtime/source ZIP and PowerShell/direct launch instructions | Five semantic suites, portable lock and 30 native render frames on actual Windows | Interactive user desktop/keyboard unverified (SSH session0); existing policy blocks ps1; direct runtime works |
| macOS | Shared native source and Terminal launcher | Shell launch contracts only | No compatible local macOS runtime, Apple host or signed .app acceptance |
| iOS/iPadOS | Explicit unsupported native status | Existing target is non-executable contract only | Linked native runtime, Apple SDK build/sign/install and actual device acceptance |

Android code generation is PBLang to JVM bytecode; local Android SDK tools package
and debug-sign. It uses Android WebView/OS services and the paired PBLang Web host.
The retained desktop VM is still a bootstrap runtime. Neither boundary is presented
as a wholly independent PBLang implementation of the operating system.

The optional Web/Android physical buttons preserve origin, CSRF, stale form and
rate controls. Tests cover no hardware operation on GET, invalid origins, replay,
changed-project rejection, Stop after edits/during preview, fixed port binding,
malformed startup project, generic errors and retention of save/import controls.
Android JavaScript opt-out is type-checked; default generator bytes remain compatible.
Ten relevant Linux semantic suites passed. Lint completed with retained Web style
warnings and a test-variable false positive; build and whitespace checks passed.
Graphify update timed out after40seconds and is not current evidence.

Actual UNO USB checks reported PLAY/running, progressing frames, finite completion,
STOP/idle and successful close/termios restoration. A real browser physical Play
returned device acknowledgement. **Visible physical LEDs remain unverified**:
the user first reported no visible output, then reported missing the short run.
Acknowledgement alone must not close that issue. No new firmware flash occurred.

Mac/iOS completion, standalone Android operation and cross-platform physical
transport remain open. No performance improvement or universal compatibility is
claimed. Existing saved26-frame content was preserved; user content is not included
in release source or these fixtures.
