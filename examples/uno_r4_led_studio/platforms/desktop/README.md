# UNO R4 LED Studio desktop source launchers

These Linux, Windows and macOS entry points run the existing PBLang native editor
from `../../view.sapp`. They preserve the shared frame model, matrix editing,
text, animation, counters, import and export. They do not connect to a board.
They are **source launchers**, not standalone executables, installers or tested
Windows/macOS releases.

Keep the complete PBLang checkout/package, including `stdlib/`, `assets/` and
`examples/uno_r4_led_studio/`. Choose a compatible PBLang 0.6 runtime built for the
host OS and architecture with working native graphics. The runtime is not
included or downloaded. A Linux executable cannot launch on Windows or macOS.
A legacy runtime may lack features required by the current source libraries.

From the repository root, use the command for your platform (replace the sample
runtime path with your installed executable):

```sh
# Linux
sh examples/uno_r4_led_studio/platforms/desktop/launch.sh /opt/pblang/pblang
# macOS, in Terminal
sh examples/uno_r4_led_studio/platforms/desktop/launch.command /Applications/PBLang/pblang
```

```powershell
# Windows, in PowerShell
& .\examples\uno_r4_led_studio\platforms\desktop\launch.ps1 -Runtime 'C:\PBLang\pblang.exe'
```

The shell launchers require an absolute executable path. PowerShell resolves the
selected executable before changing directory. All launchers use the package
root as the working directory, so relative imports and exports keep the same
meaning even when launched from elsewhere. Paths with spaces are supported.
The commands use the editor's existing `--allow-all` trusted-local-source profile;
only run a package and executable you trust. Do not change system execution policy
merely to launch the PowerShell file; where scripts are restricted, run the
following directly from the package root:

```powershell
& 'C:\PBLang\pblang.exe' run examples/uno_r4_led_studio/platforms/desktop/main.sapp --allow-all
```

The renderer uses the retained native graphics backend. Its default minifb
backend has platform-specific implementations; a runtime built with optional
SDL2 also needs its SDL2 library. The supplied launchers do not install those
libraries or select an experimental renderer.

Exports create new files under `build/uno-r4-led-studio/` in the package root.
Use a writable package directory. Import paths should be package-relative on
Windows/macOS; secure file backends have different absolute-path capabilities.
An unavailable backend reports failure rather than substituting ordinary file
writes. Actual Windows/macOS window, keyboard, fonts, import/export and counter
behavior still require acceptance checks on those hosts.

The PBLang-owned `tools/pbpackage.sapp` packager currently accepts a Linux ELF
runtime and uses Linux directory primitives. It does not create Windows `.exe`
or macOS `.app` bundles. These launchers do not invoke the legacy bundle command.

## iOS boundary

There is no linked native iOS UI runtime or IPA exporter in this checkout.
`modern_ui.compile(..., map{target:"ios"})` produces a portable contract with
`executable:false`; `modern_ui.build` rejects that target. The iOS CI job boots
a simulator and checks contracts on the macOS host; it does not install LED
Studio into iOS. A source contract or a browser preview must not be described
as an installed iOS application.
