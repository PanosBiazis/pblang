# Tagged-value refactor plan (native codegen)  —  ✅ COMPLETE

Status: landed. All sites migrated; `bench/codegen_smoke.sh` 143/143 (13 new is_*/tagged tests),
`bench/selfhost_parity.sh` 14/14. `is_string`/`is_array`/`is_map` work at runtime (TAG_STR=1, TAG_ARR=2,
TAG_MAP=3, TAG_CLO=4). Heap-object membership = single unsigned range check `(x - GBASE) <u 16MB`.



Goal: give heap objects a runtime type tag so `is_map`/`is_array`/`is_string` work → unblock the
self-hosted evaluator. Also the foundation for future dynamic-typing features.

## Layout (offset-shift: tag in a NEW leading word; length/bounds stay in a separate word)

    TAG_STR=1  TAG_ARR=2  TAG_MAP=3  TAG_CLO=4     HEAP_BASE = 0x1_0000_0000

    string  : [TAG_STR, len,   bytes@16 ]                 (was [len, bytes@8])
    array   : [TAG_ARR, len,   cap, dataptr@24] + buffer  (was [len, cap, dataptr@16])
    map     : [TAG_MAP, count, cap, pairs@24 ]            (was [count, cap, pairs@16]; pair stride 16)
    closure : [TAG_CLO, code_ptr, caps@16 ]              (was [code_ptr, caps@8])

Rationale: keep the tag and the length in DISJOINT words. High-bit tagging (tag folded into the length
word) would force every bounds check to mask the tag off first; one missed mask silently defeats memory
safety (an OOB index reads as in-range). Offset-shift traps loudly on a missed offset instead.

`is_map(x)` = `x >= HEAP_BASE && [x] == TAG_MAP` (numbers/nil are below HEAP_BASE → "not a heap object").

## Sites to update (each = an 8-byte offset shift + tag store on allocation)

Strings:  literal materialize · `s[i]` (bounds + byte load + 1-char result) · str_slice · str_concat ·
          str_from_num (itoa) · strfromcharcode · str_eq (`_emit_str_eq`) · print-heap-string
Arrays:   literal · read · write · push (+realloc) · map_keys result
Maps:     literal · map_get_str · map_set_str · map_has · map_keys (source scan)
Closures: creation (fn_lit) · indirect call (`[r11]`→`[r11+8]`) · capture read (`+8+8i`→`+16+8i`)
Shared:   `len(x)` reads `[x]` → `[x+8]`
New:      `is_map`/`is_array`/`is_string` via `_emit_is_type(tag)`

## Order & safety
len() is shared by strings+arrays, so strings+arrays+len must land together (big-bang for those), then
maps, then closures, then is_*. Run `bench/codegen_smoke.sh` (130) + `bench/selfhost_parity.sh` (14) after
each landing; a missed offset shows up as a segfault/trap, not silent corruption.
