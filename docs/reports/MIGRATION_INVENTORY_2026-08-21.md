# Bounded PBLang migration inventory

Date: 2026-08-21

## Boundary

This is a privacy-preserving, bounded inventory of the authorized project,
code, and miscellaneous-source trees. It is not a claim that every byte has
been semantically reviewed. Overlapping roots were de-duplicated because the
projects tree already contains the AAP and Spotit trees.

The `sw` tree is read-only inspiration. Its contents must never be modified,
renamed, migrated, copied into fixtures, or used to generate files.

The inventory excludes account exports, backup codes, credentials, databases,
medical or personal records, machine-local configuration, archives, binaries,
generated compiler output, package caches, dependency vendors, and build
directories. Exclusion is intentional; these are not application source.

## Existing migrations found

The PBLang examples already contain native editions for the earlier Android
exercise, Linux administration scripts, QR generation, responsive web UI,
calculator behavior, embedded circuits and control, and several polyglot
demonstrations. AIPY, Spotit, PBRemote, PBOffice, PBMedia, PBCircuits, and other
larger efforts already have product directories. New slices should extend or
consolidate these instead of creating near-duplicate projects.

## Completed in this slice

`examples/relationship_reasoning_lab` consolidates the two earlier Prolog
family-relation exercises into a bounded native PBLang knowledge lab. Synthetic
identities replace source names, while content digests preserve provenance.
Facts and reusable rules derive parent roles, siblings, grandparents, ancestors
and descendants; one responsive UI contract covers Web, native and Android
records and a local visual plan renders the relationship graph. The migration
also extends `stdlib/logic.sapp` with backward-compatible bounded query evidence
and ground equality/inequality constraints. A 96-node chain exposed and fixed
the old hidden 64-hop substitution ceiling. Cyclic recursion now demonstrates
explicit truncation rather than being presented as complete. Full Prolog
parsing, negation, cuts, arithmetic predicates and delayed `dif/2` constraints
remain outside this deterministic Datalog-style slice.

`examples/solar_daylight_planner` migrates the earlier Python astronomy plot
into a bounded PBLang-only solar and civil-time planner. It replaces empty
coordinates, an implicit timezone and four foreign plotting/ephemeris
dependencies with validated dates and observer inputs, explicit UTC offsets,
deterministic annual series, tagged polar-day/night states, and local 2D/3D
visual plans. The reusable `stdlib/astronomy.sapp` calculation returns sunrise,
apparent solar noon, sunset and daylight duration without storing coordinates
or using the network. Lunar rise/set remains explicitly unsupported until a
separately verified ephemeris exists; headless Web/native/Android records are
not presented as packaged renderer evidence.

`examples/portable_wasm_workbench` migrates the earlier eight-line C/WebAssembly
addition exercise into a bounded PBLang-only workbench. It removes Emscripten,
generated JavaScript glue and the ad hoc Python server, compiles native PBLang
source directly to a real WASM binary, checks its module envelope and `main`
export, and independently cross-checks signed addition through the bounded WASM
instruction model. The new `stdlib/wasm_module.sapp` facade centralizes source,
artifact, section and export budgets plus malformed-module rejection. One
responsive UI tree compiles to Web, native and Android records; independent
engine execution and actual packaged target applications remain explicit
verification boundaries rather than inferred support.

`examples/euclidean_gcd` migrates a small Python Euclidean-algorithm exercise.
The original behavior returned after the first loop iteration. The PBLang
edition retains the full trace, validates exact integers, cross-checks the
standard library result, and includes malformed-input regression tests.

`examples/perceptron_logic_gates` migrates the earlier C perceptron exercise
without modifying its source. The original lost bias updates passed by value
and attempted to learn XOR with one linear classifier. The native PBLang
edition adds a bounded, deterministic perceptron API to `stdlib/ml.sapp`, proves
AND/OR/NAND convergence, reports XOR non-convergence honestly, copies caller
models, and rejects malformed dimensions, labels, options, and work budgets.

`examples/angle_analysis` migrates the earlier Python degree-conversion
exercise into a bounded reusable angle model. It preserves signed input,
normalizes multi-turn angles, classifies axes and quadrants, reports undefined
tangent explicitly, and separates full-precision values from rounded display
values. The migration also corrected the PBLang library boundary by exporting
the already implemented `math.deg_to_rad` and `math.rad_to_deg` functions and
made invalid `asin` domains fail explicitly instead of manufacturing NaN.

`examples/sql_schema_lab` consolidates all 13 files from the earlier
`SQLscripts` folder into a disposable SQLite application. It replaces MySQL
administrative commands and malformed predicates with four idempotent
migrations, relational constraints, a view and index, parameterized inserts
and updates, ordered and range-filtered queries, schema inspection, and an
observed transaction rollback. The database is in-memory, fixtures are
synthetic, and no connection string or original sample identity is retained.
Dialect-specific and inaccurate prose remains documented source material, not
executable SQL or a claim of cross-database migration support.

`products/pboffice` now contains the first product-sized migration from the
earlier `WordDetect` Blazor prototype. Its native PBLang core performs bounded
XLSX package parsing and configurable identifier discovery with masked output,
synthetic fixtures, unsafe relationship rejection, and no execution of active
workbook content. This replaces the prototype's remote browser spreadsheet
dependency, duplicate root route, fixed-position layout, and hard-coded match
policy at the trusted-core boundary. Shared desktop, web, and Android UI
adapters remain a future slice and are not reported as implemented.

`examples/responsive_technology_site` consolidates both early `Webpages`
exercises into one PBLang-only Modern UI example. It replaces duplicated and
broken navigation links, fixed-width and absolute-position layouts, repeated
Bootstrap/jQuery CDN loads, duplicate DOM identifiers, undefined script state,
placeholder contact details, and personal author metadata with a shared content
model, responsive component tree, portable interactions, accessible live
status, and a local-only form demonstration. No original or remote image is
copied, and the generated Web edition has no external runtime dependency.

`examples/typed_integer_contract` consolidates both small `RustPython` crates
without carrying Rust or Python source into the PBLang tree. It preserves the
`i32` decimal-string result and the `u64` addition behavior, adds explicit range
and overflow handling, uses exact `bigint` arithmetic across the complete u64
domain, and publishes a versioned inert request/response map for future adapters.
The dependency-free Rust baseline passed in an isolated build; the PyO3 baseline
was not reproducible offline because its dependency was absent from the local
cache, so live Python-extension compatibility is not claimed.

`examples/algorithm_workbench` migrates the C sorting, searching, and Fibonacci
exercise as a bounded PBLang workbench. It adds a traced non-mutating bubble
sort, cross-checks both existing merge-sort and binary-search implementations,
requires sorted search input, and replaces unchecked C allocation, `scanf`,
integer overflow, and exponential recursion with bounded validation and exact
`bigint` Fibonacci sequences. The adjacent executables and password-bearing
files were neither read as migration input nor copied.

`examples/numerical_series_lab` consolidates the small Python pi, e, and
sum-of-squares exercises into a bounded native PBLang numerical workbench. It
removes the external `mpmath` dependency and implicit file overwrites, computes
the constants through exact `bigint` fixed-point series with guard digits,
handles signed integer squares beyond the safe-number range, and publishes an
explicit precision contract without claiming formally proved correct rounding.

`examples/pico_button_led` completes the example-level migration of the earlier
Raspberry Pi Pico MicroPython/Wokwi exercise whose generic Linux GPIO helpers
had already influenced `stdlib/gpio.sapp`. It adds an explicit safe wiring
contract, active-low pull-up semantics, deterministic contact-bounce tests,
portable RP2040 pin validation, a technical manual, and honest fail-closed
firmware status. The migration also bounds GPIO planning work and rejects
invalid or non-monotonic debounce input in the shared PBLang library. Native
RP2040 codegen, conditional input lowering, UF2 output and physical execution
remain explicitly incomplete.

`examples/bounded_increment_table` migrates the earlier C variable-length table
exercise into exact bounded PBLang decimal arithmetic. It gives `size` a real
capacity meaning, checks `times + 1` before allocation, initializes every
returned element, and rejects ambiguous or oversized input. This replaces the
source's out-of-bounds `num[size]` access, reads of uninitialized memory,
unchecked input and machine-specific build configuration without copying its
binary or object artifacts.

`examples/language_fundamentals_workbook` migrates an early multi-topic C#
console exercise into a deterministic PBLang workbook. It separates arithmetic,
Unicode text, branches, loops, attendance, collections and object behavior into
testable reports; preserves truncation-toward-zero integer division; and uses the
shared PBLang class model with correctly initialized fields. Bounded copied
arrays and explicit result maps replace unchecked parsing, input-sized
allocation, partially initialized arrays and language-specific `out` mutation.
Synthetic labels replace personal literals, and source provenance is retained
only as content digests.

`examples/quantum_circuit_lab` consolidates the small Qiskit, Cirq and Q#
experiments from the earlier quantum folder into a deterministic PBLang-only
state-vector lab. It verifies a Bell pair and six Pauli expectations, a
Hadamard random-bit distribution, square-root-of-X through `H-S-H`, and a
complete two-qubit Grover iteration without random-test flakiness, cloud
credentials, runtime package installation or foreign dependencies. Copied
Quantum Katas and licensed notebook tutorial material were classified as
third-party references and deliberately not migrated. Vendor noise models,
cloud execution and physical quantum hardware remain outside this bounded
one- and two-qubit example.

`examples/physics_motion_lab` migrates the five deterministic simulations from
the earlier Tkinter/NumPy/Matplotlib physics demo into one PBLang-only numerical
backend and shared portable presentation model. Projectile, harmonic,
small-angle pendulum, circular and travelling-wave reports accept a common
`2..512` sample budget, expose their idealized model assumptions and feed both
visual chart/timeline plans and responsive Web, native and Android UI records.
The tests cover numerical landmarks, invalid domains, bounded work and compact
layouts. Actual host animation remains a renderer responsibility and is not
claimed by the headless migration.

`examples/procedural_galaxy_scene` migrates the earlier NumPy, Matplotlib and
Tkinter galaxy scatter script into a deterministic PBLang-only procedural model.
It replaces a fixed 100,000-element random allocation with a validated
`16..4,096` work budget, a reproducible seeded generator, actual radius-coupled
spiral arms, bounded vertical spread and five compact color groups. One shared
responsive UI tree compiles to Web, native desktop and Android records, while
the same data produces a portable skybox, point-cloud and camera render plan.
The generator explicitly marks its PRNG as non-cryptographic, and headless tests
do not claim that every target currently has a live GPU renderer.

`examples/responsive_shape_gallery` migrates the fixed Win32 GDI colorful-shape
exercise into one PBLang-only responsive geometry and presentation model. It
retains the rectangle, ellipse, line, triangle and arc while removing the
Windows-only message loop, fixed 400 x 400 coordinates, clipped lower arc and
native graphics-object lifetime hazards. The migration adds validated
first-class `ellipse2d` and `arc2d` contracts to the shared visual library and
tests layouts at compact phone, desktop and ultrawide dimensions. Web, native
and Android records share one UI tree; actual host rasterization remains an
explicit renderer responsibility.

`examples/nmea_navigation_lab` extracts the useful GPS protocol behavior from
an earlier Python IP/Bluetooth experiment into a PBLang-only NMEA-0183 library
and cross-platform example. The new bounded core verifies framing and XOR
checksums before parsing, supports GGA and RMC talkers, validates coordinates,
UTC fields and calendar dates, preserves explicit no-fix states, limits batch
work and emits privacy-safe coarse views without raw sentences or device IDs.
The source's public-IP disclosure, raw Bluetooth addresses, external geocoder,
placeholder GATT UUID, infinite read loop and always-true exit branch are not
retained. Live BLE and IP-geolocation adapters remain explicitly fail-closed
until native permission-bound PBLang backends exist.

`examples/linear_barcode_studio` migrates the earlier Python barcode generator
into a bounded PBLang-only linear-barcode library and responsive example. The
shared core validates and encodes Code 128 Set B, EAN-13 and UPC-A, generates or
verifies GTIN check digits, publishes canonical module streams and merged bar
runs, and renders local SVG and bounded PNG output. Complete independent module
vectors, PNG decode round-trips, malformed input, compact Web/native/Android UI
and a 192-symbol workload are covered by tests. The external Python dependency,
fixed output basename, duplicate `.png` message and implicit arbitrary input are
not retained. GS1-128, FNC1, Code Sets A/C, assigned identifier ownership,
scanner certification and print-quality verification remain explicitly outside
this slice.

`examples/supervised_tasks` migrates an earlier Python daemon-thread exercise
into a bounded PBLang-only structured lifecycle. The upgraded example validates
all workload limits before spawning, runs a finite worker, cooperatively cancels
the background worker, verifies zero active, detached and unreaped work, and
deterministically reaps both terminal records. Repeated lifecycle tests cover
race-prone cleanup while one responsive UI tree compiles to Web, native and
Android records. Silent daemon abandonment and the unsynchronized global stop
flag are not retained. Preemptive VM-thread termination and isolation of
unbounded native calls remain explicitly unsupported.

`examples/file_catalog_dashboard` migrates an earlier Python folder-analysis
and charting script into a bounded PBLang-only metadata catalog. A reusable
library classifies more than one hundred specific and multipart suffix rules
with deterministic precedence, explicit ambiguous labels and aggregate-only
privacy defaults. One-level scans reuse the existing metadata-only inventory
boundary, while accessible rows and portable bar, donut and radial-column plans
replace NumPy and Matplotlib. Tests cover overlap regressions, malformed and
oversized inputs, non-recursive scanning, privacy, compact Web/native/Android
layouts and a 2,048-item workload. Magic-byte detection, content inspection,
archive traversal and recursive classification remain explicitly unsupported.

`examples/vehicle_emissions_lab` migrates an earlier Python fuel-consumption
analysis script into a bounded PBLang-only data workflow. It replaces a
machine-specific dataset path and five foreign data/plotting dependencies with
the native dataframe, statistics, machine-learning, visual and Modern UI
libraries. The migration validates CSV shape and work budgets, reports schema,
missing values, summaries, category-encoding dimensions and correlations,
performs a deterministic train/test regression with measured MSE/RMSE/R2, and
shares one responsive Web/native/Android presentation contract. Raw rows,
category labels and personal paths are excluded by default. The model is
honestly scoped to the strongest single numeric feature; multivariate and
categorical-coefficient modeling remain outside this example.

`examples/package_version_inspector` migrates an earlier Python module-version
prompt into bounded, non-executing PBLang package inspection. Instead of
importing arbitrary user-selected modules and triggering their initialization,
the new shared library validates supplied PBLang manifest text and inert
catalog records, evaluates semantic-version requirements, reports current,
outdated, missing, incompatible and ahead states, and hides dependency names by
default. A backward-compatible strict SemVer API now rejects malformed release
identifiers without changing the legacy permissive parser. The responsive
Web/native/Android example performs no process execution, arbitrary path read
or live registry request; signature verification and installed-artifact
attestation remain separate permission-bound capabilities.

`examples/native_document_studio` migrates an earlier Python PDF creation and
inspection script into a bounded native PBLang document pipeline. A reusable
writer now emits a real classic-xref PDF with text, a two-column table, an
ASCIIHex-encoded RGB image and one HTTPS link, while an independent structural
inspector validates and recovers its page, object, title and rendered-content
contract. One responsive UI tree compiles for Web, native and Android, and an
explicit project-relative exporter verifies saved bytes by reading them back.
Personal storage paths, automatic browser launch, remote image dependencies,
author and creator metadata are not retained. Arbitrary PDF parsing, multiple
pages, encryption, object streams, embedded fonts, forms, signatures and PDF/A
certification remain explicitly outside this strict first subset.

`examples/vehicle_motion_playground` consolidates earlier 2D and unfinished 3D
Go car exercises into one deterministic PBLang-only simulation. It replaces
frame-dependent movement, a missing image dependency, fixed non-responsive
windows and the empty 3D draw function with bounded fixed-step timing,
acceleration/reverse/steering/braking, world collisions, procedural 2D geometry
and a complete 3D body/cabin/wheel render plan. A single accessible UI tree
compiles to Web, native and Android records, with keyboard/touch-neutral control
commands and no external assets or foreign game runtime. Physical GPU,
browser-canvas and Android-device rendering remain host-backend verification
work and are not inferred from the passing headless plan tests.

`examples/secure_configuration_inspector` migrates an earlier Python dotenv
loader into a bounded PBLang-only configuration workflow. The original source
loaded the current-directory `.env` implicitly and printed a database URL. The
migration instead validates supplied text through a new backward-compatible
strict parser, rejects malformed and duplicate entries, classifies sensitive
field names and produces serialization-safe redacted reports. Required/type
diagnostics and one responsive Web/native/Android UI contract are covered by
tests without reading host environment variables or files. Live service
connectivity, credential authenticity and secret rotation remain explicit
permission-bound responsibilities rather than inferred capabilities.

`examples/bounded_loopback_service` migrates an earlier Python socket listener
into a PBLang-only local connection and lifecycle example. It replaces a public
wildcard bind, indefinite accept and peer-address logging with loopback-only
ephemeral binding, bounded JSON frames, explicit accept/read/write/connect
deadlines and verified descriptor cleanup. Pure protocol tests and one
responsive Web/native/Android status tree remain portable; a real round trip
uses the existing PBLang-native Linux deadline adapter and fails closed on
unsupported hosts. Remote TLS, authentication, service discovery and
multi-client operation remain outside this deliberately local contract.

`examples/mips_assembly_workbench` migrates three small MIPS teaching sources
from the authorized code archive into one PBLang-only parser, encoder, executor,
and responsive Web/native/Android presentation contract. The migration uses
real 32-bit MIPS encodings, signed overflow checks, aligned sparse word stores,
bounded source and execution budgets, and content hashes for source identity.
One source used non-standard two-operand `addi`; strict mode rejects it, while
the explicit compatibility mode normalizes each affected line and publishes
warnings. The example stores no original source text or personal paths and does
not invoke an external assembler, process, or network service. Labels,
directives, macros, branches in source text, object formats, full operating-
system ABIs, and general MIPS emulation remain outside this verified subset.

`examples/x64_message_abi_lab` consolidates four small x86-64 message-output
exercises into one PBLang-only ABI and artifact workbench. The original Linux
variant maps to a real static ELF64 artifact with direct `write` and `exit`
syscalls and an RX load segment. A new shared facade also emits a real Windows
PE32+ artifact with bounded Kernel32 imports, correct argument registers,
32-byte shadow space, aligned stack allocation, NX metadata and separate code
and data sections. Three historical variants mixed SysV registers into Windows
calls or omitted required call-frame setup; normalized descriptors now report
those defects without retaining or executing source text. Original messages
and personal paths are excluded, while four SHA-256 digests preserve source
identity. Windows structure is verified on this Linux host, but live Windows
execution and ASLR are not claimed because the bounded PE has no relocations.

`examples/machine_code_safety_lab` migrates six historical Windows batch
recipes that attempted to execute raw machine-code bytes. The original intent
(`return 42`, `return 100`, and `5 + 5`) is preserved through bounded shared IR
and native PBLang artifact backends for DOS 8086 COM, Linux x86-64 ELF and
Windows x86-64 PE. The migration identifies writable-executable memory,
shell/reflection execution, temporary binaries, malformed hexadecimal data,
an x86/x86-64 container mismatch, an incorrect DEBUG byte count and an `INT
20h` exit-code mismatch. Historical recipes are inspected as data and never
executed. Source identity uses six SHA-256 digests without retaining personal
paths or original script text. Linux execution, the DOS bounded model and PE
structure are covered by their existing backend gates; live Windows execution
remains explicitly unverified.

`products/pbspotit` is the first product-sized migration selected from the
standalone SpotIt Android project. The completed slice moves its accelerometer
decision logic into `stdlib:motion_detection.sapp` and improves it with
caller-supplied monotonic nanoseconds, strict time ordering, bounded work,
three-axis gravity-vector filtering, raw-sensor warmup and deterministic
replay. The product adds monitoring/alarm state, stable local event IDs, a
30-entry history ceiling and one responsive UI contract compiled for Web,
native desktop and Android records. Two source SHA-256 digests retain
provenance without storing source text, personal paths or author metadata.
Android sensor registration, foreground execution, location, SMS,
siren/vibration, secure settings persistence and package installation remain
explicitly unavailable; cross-target UI compilation is not claimed as live
platform execution.

## Candidate queue

### Small examples

1. Numerical constant and series exercises: migrate only after selecting a
   bounded precision contract and `bignum` representation.
2. Small C, C++, Lua, and Python algorithm exercises: group by behavior and
   create one tested PBLang example per distinct algorithm rather than one per
   source filename.
3. Remaining static web exercises: compare them against
   `responsive_technology_site` and migrate only distinct behavior, using the
   shared responsive PBLang UI model, accessibility checks, and no remote
   trackers.
4. Remaining graphics and astronomy exercises must continue to separate
   deterministic calculation from rendering. The reviewed rise/set plot now
   has a bounded solar/civil-time migration with explicit coordinates,
   timezone and polar states. Verified lunar ephemeris depth and live renderer
   validation remain separate future slices.
5. Password-generation exercises are deferred. The current VM implementation
   documents `random_bytes` as non-cryptographic while credential helpers claim
   cryptographic randomness. A credential migration requires a separately
   reviewed operating-system entropy contract, unbiased sampling, secret-safe
   output, and no default plaintext persistence.

### Product-sized candidates

1. Continue PBSpotIt with one verified Android effect boundary at a time,
   starting with sensor registration and lifecycle-safe foreground execution.
2. CRUD variants should become one reference product after their schemas,
   validation rules, and workflows are compared.
3. Additional word-detection and document utilities should extend PBOffice's
   bounded reader and policy layers instead of carrying project-local parsers.
4. Raspberry Pi and LED experiments should extend the existing embedded
   examples and board descriptors rather than form a second hardware stack.
5. Django, Laravel, Java, C#, and desktop-GUI projects require individual
   behavioral baselines before native PBLang replacement.
6. Large trees containing copied frameworks or generated sources require an
   ownership/dependency split before application code can be selected.

## Per-slice acceptance gate

Each migration must:

1. identify the true source and exclude generated/private artifacts;
2. describe existing behavior and known defects;
3. add a native PBLang implementation without foreign source fallback;
4. add positive, boundary, malformed-input, and regression tests;
5. run the normal parser/type checker, linter, and focused runtime tests;
6. scan the new files for personal paths and credential-shaped content;
7. document unsupported behavior without claiming universal portability.

The next slice should remain small. Runtime verification that imports the live
stdlib directly must wait until the current PBLang root lock is synchronized
with the separate in-progress process-lifecycle work. The perceptron regression
was therefore also executed in an isolated package mirror containing the exact
current `stdlib` source; the root lock was deliberately left unchanged.
