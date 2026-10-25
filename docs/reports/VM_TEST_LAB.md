# pblang cross-platform VM and emulator lab

The lab validates two independent claims:

1. pblang builds and behaves correctly on a real host operating system.
2. a pblang instruction model agrees with an independent reference emulator.

A VM result does not by itself prove an ISA emulator correct. A reference-oracle
result does not prove Windows, macOS, Android, or another host integration works.

## Evidence levels

| Label | Meaning |
|---|---|
| `native-host` | executed directly on the development host |
| `native-vm` | built and executed inside a hardware-accelerated guest OS |
| `hosted-vm` | built and executed on a pinned hosted CI runner |
| `simulator` | executed in Android Emulator or Xcode Simulator |
| `userspace-emulation` | guest userspace executed through another architecture model; no physical-board claim |
| `reference-oracle` | bounded architectural state compared with an independent implementation |
| `adversarial-vm` | isolated hostile-input testing; never a release baseline |

Physical Raspberry Pi, Android, and iOS devices remain separate evidence classes
until a device-runner contract records their exact hardware and boot identities.

## Safety architecture

The operational controller must not run from the working tree it tests. The
reviewed source is bundled from a clean worktree, content-addressed, installed
outside the repository, and checked before every run. The active repository is
uploaded only as bounded untrusted data.

Every local VM record binds all of:

- domain name and UUID;
- canonical qcow2 path and baseline digest;
- MAC address and isolated network UUID;
- private pinned IP address and strict SSH host key;
- distinct forced-command control and SFTP-only transfer credentials.

Any mismatch stops the matrix. Friendly domain names alone are never authority.
Guests run sequentially. Golden images are read-only and each run uses a fresh
qcow2 overlay. Test networks have no forwarding; dependency downloads occur only
during a separate baseline-maintenance phase. Kali is always isolated and has no
personal credentials, shared folders, clipboard channel, USB redirection, guest
agent, passthrough device, or nested virtualization.

The fixed guest agent accepts only a content-addressed job identifier. Suite IDs
come from `stdlib/vm_lab.sapp`; manifests cannot provide command text. Linux jobs
run as an unprivileged account in a bounded service/cgroup. Windows requires a
Job Object with kill-on-close semantics; FreeBSD requires a jail/rctl or an
equivalent verified supervisor. Cleanup uncertainty quarantines the guest and
stops the matrix.

## Source bundle contract

The bundle contains sorted regular files selected by Git's tracked plus
non-ignored-untracked view. It rejects symlinks, hardlinks, devices, FIFOs,
sockets, absolute paths, `..`, backslashes, NUL, and excessive path depth.
It applies fixed file-count, per-file, and total-byte ceilings. `.git`, `target`,
`graphify-out`, VM images, reports, environment files, keys, and certificates are
excluded. The manifest records each relative path, size, and SHA-256 plus a bundle
digest. The guest verifies the complete manifest before extraction.

## Reports and release evidence

Guest reports are hostile input. The controller downloads only a fixed path,
hashes bytes before parsing, applies size/depth/count limits, validates the exact
schema, and removes terminal control characters before display. Reports contain
opaque target IDs and repository-relative paths; never credentials, usernames,
IP addresses, personal absolute paths, environment dumps, or raw command lines.

The host—not the guest—constructs `platform_emulator.release_conformance()`
evidence after recomputing every digest. Evidence is bound to source revision and
dirty-state digest, controller digest, domain/baseline/boot identities, suite
records, timestamps, and cleanup verification. Guest-created all-green booleans
are not trusted.

## Matrix

The canonical resource-safe order on a 16 GB host is:

1. Linux Mint native smoke tests.
2. Debian 13 x86-64 KVM guest.
3. Windows 11 x86-64 KVM guest.
4. Windows 10 x86-64 legacy KVM guest.
5. FreeBSD 14 x86-64 KVM guest.
6. Android API 34 x86-64 accelerated emulator.
7. Raspberry Pi OS ARM64 userspace/QEMU target.
8. pinned macOS runner and Xcode iOS simulator.
9. ISA reference-oracle campaigns.
10. isolated Kali adversarial guest.

Only one local guest or mobile emulator runs at a time.

## Raspberry Pi OS and ARM64

The local Raspberry target has two intentionally different gates:

- QEMU `raspi3b` boots a checksum-verified Raspberry Pi OS Lite ARM64 image far
  enough to mount the real root filesystem and start systemd as ARM64. QEMU's
  Pi model does not fully implement VideoCore firmware, GPIO, Wi-Fi, or timing,
  so physical-board behavior is not claimed.
- `tools/pbemucheck.sapp` generates Linux AArch64 ELF files with pblang,
  creates them exclusively inside a private random per-run directory, measures
  them before and after execution and removes only an unchanged artifact after
  the bounded fixed `/usr/bin/qemu-aarch64-static` run. Each seeded input,
  artifact, exit and output digest is retained in a content-addressed,
  self-reported claim. Because QEMU still receives a mutable pathname,
  artifact/result binding, same-object handoff,
  QEMU publisher/digest authenticity and oracle independence are not verified.

Run the structural code-generation comparison with:

```sh
pblang run tools/pbemucheck.sapp --allow-all
```

## Android and iOS

Android uses the dedicated `PBRemote_API_34` AVD. A release gate rebuilds the
APK, boots a wiped headless emulator, installs and cold-launches the activity,
checks the accessibility tree and interaction result, scans the crash buffer,
then shuts the emulator down. This is Android API 34 x86-64 simulator evidence,
not a physical ARM phone or vendor-firmware claim.

The hosted workflow repeats the same API 34 interaction gate on an ephemeral
Linux runner. It retains the APK digest, before/after accessibility trees,
individual command-log digests, emulator cleanup checks, and a content-bound
`pblang.vm-lab.hosted-report.v2` report.

iOS cannot run as a generic KVM guest. `.github/workflows/vm-lab-hosted.yml`
uses a pinned macOS hosted runner, dynamically selects an available Xcode iOS
runtime, boots a temporary simulator, validates pblang's iOS-facing contracts,
records the runtime, and deletes the simulator. The workflow does not claim a
physical iPhone or a native iOS application backend that pblang does not yet
provide.

## Offline dependency baseline

Native guest jobs never download dependencies. The Debian baseline must contain
the exact Cargo registry inputs selected by `Cargo.lock`; the per-job bundle is
reserved for source-under-test and remains capped at 64 MiB. If the lock changes,
run a separate network-enabled maintenance build, verify the fetched inputs,
disconnect the maintenance network, re-enable the guest firewall, and publish a
new baseline digest before any test job is accepted.

Windows installation remains a manual-license boundary: use an official Windows
evaluation ISO, finish Microsoft's required download and installation flow,
enable OpenSSH Server, and only then enroll the VM's real UUID, disk, MAC, host
key, and baseline digest. Placeholder identities are never runnable.

Windows 11 is the supported Windows release gate. Windows 10 is retained only
as a legacy-compatibility target because its normal Microsoft support has ended;
it cannot promote a release by itself. Both guests use independently verified
Microsoft media and separate immutable baselines.

FreeBSD uses the official 14.4 BASIC-CLOUDINIT UFS image. Its network-enabled
maintenance overlay is disposable and host-filtered; only a separately booted,
offline validation overlay may be promoted to the isolated test baseline.

## Source-tree commands

Copy `vm-lab.example.json` to the local private configuration location and
replace placeholder identities during provisioning:

```text
~/.config/pblang/vm-lab.json
```

The source-tree command is intentionally read-only:

```sh
pblang run tools/pbvmtest.sapp --allow-fs -- doctor
pblang run tools/pbvmtest.sapp --allow-fs -- list
pblang run tools/pbvmtest.sapp --allow-fs -- plan enabled
```

`run`, `emulators`, and `report` are available only from the reviewed immutable
controller artifact installed by the provisioning workflow.

## Acceptance gates

- swapped UUID, disk, network, MAC, baseline, or host key is rejected;
- config shell metacharacters cannot reach a local or remote shell;
- unauthorized domains cannot be started, stopped, reverted, or destroyed;
- fork/daemon fixtures are terminated inside every supported guest;
- test guests cannot initiate host or internet connections;
- traversal, links, special files, oversized bundles, and forged digests fail;
- malformed, deep, oversized, or control-sequence reports fail safely;
- interrupted runs recover deterministically or quarantine the guest;
- privacy scans find no credential or personal-path material;
- exact-release evidence is created only after shutdown and cleanup verification.

## Pinned external QEMU maintenance campaign (2026-09-10)

`tools/pbemucheck.sapp` still defaults to the fixed system AArch64 QEMU.
A trusted maintainer can instead provide both `PBLANG_QEMU_AARCH64` (absolute
executable path) and `PBLANG_QEMU_BASE64_SHA256` (64 lowercase hexadecimal
characters). The digest is **SHA-256 of the Base64 encoding of the executable
bytes**, matching `linux_process.prepare_fixed_verified`; it is not the usual
raw-byte `sha256sum`. Neither value is written into the differential report.

The optional executable is copied into a sealed snapshot, measured once, held
across the five fixed benign cases and duplicated for each bounded launch.
No caller IR, source or child arguments are accepted. A bad/missing pair rejects
before artifact creation; a digest mismatch rejects before any case execution.
Each child must terminate and clean its owned state before ELF inspection or
removal. Uncertain states get one bounded recovery attempt; unresolved recovery
retains artifacts and fails the campaign. Final prepared-descriptor close gates
success. Reports use the explicit `pblang.emulator-differential.v2` schema; legacy v1
claims must be regenerated and are never silently upgraded.

Child limits are 7 seconds wall time, 3 CPU seconds, 512 MiB address space,
64 descriptors, 4096 captured output bytes and 1 MiB file output. The 2048 task
limit is shared by the host UID, as required by RLIMIT_NPROC semantics; it does
not establish a private descendant budget. Case durations are rounded upward
to integral milliseconds for the differential-report contract. This campaign
proves only the selected external computations. Publisher authenticity, oracle
independence, full ISA coverage, guest OS coverage and physical-device support
remain unverified. See `RELEASE_QEMU_VALIDATION_2026-09-10.md` for current results.
