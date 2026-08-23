# Native Document Studio

This PBLang-only example migrates the behavior of an earlier small PDF script
into a deterministic, bounded and portable document workflow. The original
source is identified only by its SHA-256 digest:

`a28797b11953392e8b84a6253f15be1d8989ce64c800bbcb16307bf081df0205`

## What it does

- creates a real one-page PDF with text, a two-column table, an embedded RGB
  image and one HTTPS link;
- validates the generated PDF header, classic xref, object graph, title,
  rendered text, image and link counts;
- shares one responsive UI tree across Web, native desktop and Android;
- omits author, creator, personal paths and remote image dependencies;
- exports only through an explicit project-relative action and verifies the
  saved bytes by reading them back.

From the PBLang repository root:

```text
pblang run examples/native_document_studio/main.sapp
pblang run examples/native_document_studio/export.sapp --allow-all
pblang run examples/native_document_studio/test.sapp
```

The explicit export writes
`examples/native_document_studio/generated/document-composer.pdf`. The
`generated` directory is disposable output and is not required by the example.

## Honest limits

The native library intentionally supports a strict one-page subset with
printable ASCII, Helvetica, two-column tables, RGB images and HTTPS links. It
does not claim arbitrary PDF parsing, encrypted output, embedded fonts,
incremental updates, object streams, JavaScript, forms, signatures or PDF/A
certification. Unsupported content fails clearly instead of being reported as
successfully generated or inspected.
