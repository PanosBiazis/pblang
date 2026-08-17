# Media generation

`stdlib:media_generation.sapp` provides bounded media primitives written in
PBLang:

- deterministic procedural RGBA image generation;
- PCM16 tone and UI sound-effect synthesis;
- deterministic instrumental composition;
- bounded YUV4MPEG2 animation generation; and
- credential-free, explicit-consent model request/response envelopes.

The procedural generators hash prompts into deterministic parameters. They do
not perform semantic neural inference and expose
`semantic_prompt_interpretation=false`. Model handoffs never contact a provider;
they bind a returned artifact to the exact request digest and validate canonical
Base64, MIME type, size, and basic artifact structure.

```pblang
let media = import("stdlib:media_generation.sapp");
let generated = media.generate_image("blue geometry", 320, 200, "mosaic");
let png = import("stdlib:image_pipeline.sapp").encode_png(generated.image);
fs_write_bytes("generated.png", png);
```

Applications remain responsible for capability policy and secure atomic output
publication. The PB-MCP integration provides those boundaries.
