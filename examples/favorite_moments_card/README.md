# Favorite Moments Card

A PBLang-only, responsive web example rebuilt from a small one-file HTML
exercise. It preserves the original learning concepts while replacing invalid
document metadata, non-portable paths, inaccessible controls, and placeholder
interactions with the shared PBLang Modern UI contract.

## Included behavior

- responsive two-column and compact mobile layouts;
- local, privacy-safe image assets with accurate alternative text;
- Like, Post, letter-selection, Accept, and Learn actions;
- accessible status announcements and minimum-size controls;
- strict local web defaults with no external requests or telemetry; and
- deterministic source provenance through the original SHA-256 stored in
  `app.sapp`.

The provided image is copied into `assets/open-source-mark.png`. No personal
source path is retained in the application, generated site, or documentation.

## Run

From the PBLang repository root:

```sh
pblang run examples/favorite_moments_card/main.sapp --allow-all
```

Open `examples/favorite_moments_card/build/web/index.html` in a browser. The
generated site is self-contained and does not require a server.

## Verify

```sh
pblang run examples/favorite_moments_card/test.sapp --allow-all
pblang lint examples/favorite_moments_card/app.sapp
```
