# Palette and color workflows

## Implementation and origins

PBPalette is a local color worksheet, built from existing colors, image pipeline, tables, modern UI and text-page modules. PBStencil now reuses the same strict colors for preview/artwork export while retaining binary-mask interchange. The product idea combines the responsive-shape-gallery and galaxy/color group inventory in `MIGRATION_INVENTORY_2026-08-21.md` with an original small palette-to-Sheets workflow. These are inspiration pointers from existing scan documentation; no new broad filesystem content scan or third-party code import is represented by this slice. All new product/library implementation is PBLang.

## Shared contracts

- `colors.rgb_checked`: detached exact r/g/b map; finite integer channels 0–255, no extra/alpha fields.
- `colors.hex_checked`: exact opaque #RGB or #RRGGBB; no whitespace or unprefixed values.
- `colors.contrast_checked`: ratio and AA/AAA normal/large flags from full precision opaque sRGB values.
- `colors.best_text_checked`: choose whichever of black/white gives greater contrast; return both candidate ratios; ties choose black.
- `colors.contrast_matrix_checked`: up to 32 RGB values, validate and calculate luminance once per color, fill symmetric pairs, independent output rows and no input mutation. Empty input returns an empty matrix. No persistent cache.
- `binary_mask.rgba_colors`: validate mask and colors once, map selected/unselected pixels to independent opaque RGBA arrays. Existing `rgba` remains white on black.
- `palette_document`: strict `pblang.palette.v1` document, 1–8 swatches, 16 KiB encoded input, canonical lowercase #RRGGBB, single-line nonblank labels/title bounded to 40 characters/80 bytes, exact schema and duplicate-key rejection. Copy/configure/set/append/remove retain detached ownership. Analysis, matrix, PNG strip and six-column sheet exports derive from the same document.
- `palette_view`: shared metrics reader; color-selection actions replace detail text consistently across native/Web/Android compiler targets. Label wrapping reserves four lines. Display truncation is not used in threshold comparisons.

Legacy permissive color APIs retain their behavior. An old open-ended HEX string slice was expressed with explicit `str_slice` bounds to satisfy current lint; legacy color regression suites pass.

## Numerical reference

The checked path uses the current sRGB transfer boundary 0.04045 and relative-luminance coefficients 0.2126/0.7152/0.0722. The legacy boundary is retained for compatibility (equivalent for 8-bit channel values). Contrast is `(lighter+0.05)/(darker+0.05)`. Black/white gives 21:1; #777777 on white is approximately 4.47808945 and fails the normal-text AA threshold 4.5, despite possible misleading rounding.

References: [W3C relative luminance](https://www.w3.org/TR/WCAG22/#dfn-relative-luminance), [W3C contrast minimum](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). Pair metrics do not certify an application's accessibility. Alpha compositing, P3/HDR, font context, APCA and automatic theme changes are not implemented.

## Product surfaces

PBPalette: native palette editing, eight-step undo, draft guards, exclusive new-copy persistence; CLI creation/inspection/matrix/pair/export; PNG strip and CSV/Sheets interchange. File failures are structured; CLI clients inspect `ok`, not only exit status. Web exposes precomputed metric selection. Android reader classes are generated; neither portable edition has file editing/persistence parity. Native Linux was tested in an isolated display; Windows/macOS and installed mobile releases remain unverified. Fixed native workspace: 1100×1100.

PBStencil: color settings are transient preview/export state, outside the recipe and undo baseline. Colored PNG is explicit artwork (`selection_mask:false`); legacy binary PNG is preserved. The native workspace is 1100×1150. Existing portable geometry previews remain unchanged. Both products require fresh output names and report that exclusive copy is not atomic/durable; a failed write can leave a partial new file.

## Verification and benchmark

Twelve focused/regression suites cover malformed input, exact contrast boundaries, all 256 grayscale best-text choices, 32×32 matrix equivalence/symmetry, detached ownership, strict document round-trips, all-eight portable selection actions, native callbacks/draft/undo, no-overwrite file behavior, decoded PNG pixel equality and existing Cleanroom/PBStencil interchange. Both manifests and builds pass strict validation in an isolated dependency snapshot; lint exits successfully with unused-variable warnings. Native window captures and live browser selection validate visible behavior; generated Android classes are not installation evidence.

Run `pblang run bench/contrast_matrix.sapp --allow-all`. It compares a repeated checked scalar-pair matrix against the checked batched matrix, verifies identical results, uses two warmups and seven wall-clock samples. Median/p95 milliseconds from the local retained runtime:

| Colors | Repeated median / p95 | Batch median / p95 | Median ratio |
|---|---:|---:|---:|
| 8 | 2.815 / 2.884 | 0.380 / 0.397 | 7.42× |
| 16 | 11.003 / 23.512 | 1.029 / 4.102 | 10.69× |
| 32 | 43.398 / 115.724 | 3.246 / 3.503 | 13.37× |

p95 uses nearest rank (maximum of seven). Other work was running on the host: these are operation-specific observations, not whole-application or cross-platform speed guarantees. Both versions still require O(n²) output; the batch avoids repeated validation and nonlinear channel conversion.
