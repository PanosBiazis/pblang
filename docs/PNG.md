# Bounded PNG Decoding

`stdlib:png.sapp` decodes PNG image bytes entirely in PBLang. It is intended for
untrusted or remote GUI images where shelling out to an external converter or
using a shared temporary file is not acceptable.

```pblang
let PNG = import("stdlib:png.sapp");

let result = PNG.decode(bytes, map{
    max_input_bytes: 98304,
    max_pixels: 2073600,
    max_output_width: 320,
    max_output_height: 200
});
if not result.ok { throw result.error; }
let pixels = result.image;
```

The decoder verifies the PNG signature, every chunk CRC, the zlib Adler-32,
dimensions, exact decompressed size, exact zlib stream termination, filters and
terminal `IEND`. It supports
non-interlaced 8-bit grayscale, RGB, indexed `PLTE`/`tRNS`, grayscale-alpha and
RGBA images. Unknown critical chunks, palette overflows, interlacing, excessive
dimensions, trailing bytes and decompression bombs fail closed.

Output can be downsampled while scanlines are decoded, avoiding allocation of a
full output pixel grid. The inflated source scanlines still count against
`max_pixels`, so output limits do not weaken the decompression-bomb boundary.
Chunk CRCs operate directly on bounded byte ranges without copying chunk data.
The DEFLATE decoder uses a bounded 9-bit Huffman lookup where possible and the
canonical bitwise decoder for longer codes. Scanline filters use specialized
loops, and downsample column indexes are computed once per image.

`Gui.image` accepts an optional callback:

```pblang
fn clicked(form, local_x, local_y) { /* bounded coordinate handling */ }
Gui.image("preview", 0, 0, 320, 200, pixels, clicked);
```

Image upscaling draws source-sized nearest-neighbour blocks. This makes work
proportional to source pixels rather than HiDPI viewport pixels.
