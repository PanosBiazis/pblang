# Palette to stylesheet handoff

PALETTE_AND_COLOR_WORKFLOWS traces the original palette idea to the responsive
shape/color inventory. CATALOG_INTAKE_COUNTS_2026-09-17 is a bounded historical
metadata sample, not complete disk coverage. This extension adds an original,
small palette-to-web handoff, reusing existing palette validation, strict colors,
secure exclusive file output and modern_ui. No new disk scan or third-party
source import is represented by this work.

`palette_stylesheet.tokens(doc)` validates the complete canonical palette and
returns detached ordered name/value records. The fixed background/on-background
pair is followed by one color/on-color pair for every 1-based stored swatch.
`encode(doc)` emits only a fixed :root selector, fixed token syntax and validated
canonical HEX values, ending with LF. Maximum 8 swatches / 18 declarations and
1024 output bytes. Arbitrary title/label strings never enter syntax or comments.
Changing names therefore leaves output equal; changing order changes numbered
values. There is no persistent cache, source mutation, CSS parser or URL feature.

Black/white choices use colors.best_text_checked, including its tie rule. No new
contrast formula, threshold or performance claim is introduced. Pair calculations
are not a whole-site accessibility audit. CSS supplies opaque sRGB custom
properties; it does not target native/Android widget styling or apply a theme to
another project. Applications control which properties they consume.

PBPalette exposes the same exporter from CLI and native CSS tokens button.
Pending drafts block export; derived output does not clear the editable palette
baseline or history. Existing-file and invalid-input failures retain sources.
Web/Android remain metrics readers. Greek examples generate three independent
stylesheets, source palettes and worksheets for self-service demonstration.

Privacy/security scope: output omits labels/title, but color choices/order can
still be sensitive. No new network/authentication/telemetry or third-party
package. Existing secure_file exclusive-new behavior and non-atomic-copy caveat
are retained; failed writes may leave a partial new file. Focused source and
boundary checks do not certify the whole runtime or all dependencies.

Verification includes exact CSS bytes, every grayscale text choice, 8-color
bounds, unique tokens, independent black/white ratio comparison, ordering,
detached outputs, malformed HEX, CSS-shaped labels, native draft/dirty/history,
exclusive output and existing-format regressions. Physical OS/device execution,
installers and hosted CI are separately evidenced, never inferred from generation.
