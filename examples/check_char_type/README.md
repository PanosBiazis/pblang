# Character Type Example

This example evolves Panos Biazis's original C school exercise into a safe,
Unicode-scalar-aware PBLang program.

It classifies every character as lowercase, uppercase, decimal digit,
whitespace, control, emoji, mathematical symbol, ASCII punctuation, or another
Unicode scalar. Strings are indexed by Unicode scalar rather than UTF-8 byte,
so Greek text and emoji are not split into invalid bytes.

Run it from the PBLang project:

```sh
./target/release/pblang run examples/check_char_type/main.sapp
```

The reusable API is in `stdlib/unicode.sapp`:

```pblang
let Unicode = import("stdlib:unicode.sapp");
print Unicode.classify_character("λ");
print json_encode(Unicode.classify_text("A٣😀"));
```

The library rejects empty or multi-character input to
`classify_character`. Scripts without upper/lower case are reported as
`unicode`; the API does not pretend to implement every Unicode general
category without a versioned Unicode Character Database.
