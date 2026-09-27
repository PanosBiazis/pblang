# Reproducible study-deck selection

Extends the existing study/lesson workflow and the separate randomization backlog in `STUDY_WORKSHEETS.md`, reusing the shared random and deck libraries. This is a new composition, not a new disk scan or copying of discovered private content.

- `seeded_order.permutation(count,seed)` provides bounded reusable index ordering.
- `study_mix.create(deck,count,seed)`, `parse_count`, `parse_seed` and `summary` support independent deck preparation. `summary` expects an internally produced report; it is not a validator or a log-safety guarantee.
- `study_mix_files.open/run` reuse `study_files` for bounded reads and exclusive new-copy writes.
- `study_mix_view` shares the native preparation form and portable metadata overview between PBStudy and PBDeckMix.

The complete source deck is validated before selecting. Count is 1..number of cards (maximum 64), seed is an integer 0..4294967295. CLI fields accept only 1–10 unsigned ASCII decimal digits, including leading zeros; no whitespace, signs, fractions or exponents. The same source order/count/seed yields the same selected order. Reusing a seed after changing the source order or membership does not preserve selection identity.

`seeded_order.permutation` reuses the existing local `random.seed` and Fisher–Yates `shuffle` over indices, bounded to 1..4096 indices. Its LCG uses multiplier 1664525, increment 1013904223 and modulus 2^32. No clock or global RNG is used. This is deterministic pseudorandom ordering, not cryptography, an unbiased sampling guarantee, secure examination randomization or a learning-effectiveness claim.

`study_mix.create` first makes a validated detached copy, selects the first count entries of the permutation, and retains original title, IDs, prompts, answers, stages, due dates, review counts and last-review dates exactly. IDs need not be consecutive and are not renumbered. The result includes selected source positions (1-based) and IDs. Empty selection rejects; a one-card selection/deck is supported. All original study content/serialization limits remain.

PBStudy continues to sort due reviews by date. Mixed saved order changes tie order for equal dates; it does not override older dates or make future cards due. Exported subsets are independent copies and do not synchronize subsequent progress to the original. To combine a mix with existing due-only selection, explicitly export a due deck first and mix that copy. No hidden due filtering is applied.

Native preparation rereads the selected file when Mix is pressed. Both metadata preview and saved copy refer to that captured deck; external file changes are not automatically detected. Changed source/count/seed fields require Mix again. Failed preparation retains an explicitly marked prior selection and disables saving. Output must be a new .pbdeck; failed writes may leave an incomplete new file. No atomic visibility/crash durability promise.

Portable snapshots contain seed, counts, selected IDs, due dates and stages, but no deck title, question or answer text. Those identifiers/dates are not automatically harmless to disclose. Saved .pbdeck copies contain full selected study content and history. No network, telemetry, automatic scan, hardware operation or external dependency is added.

Tests include twenty independent integer-LCG/Fisher–Yates fixtures, a 4096-index permutation, 256 count/seed deck cases, complete history retention, detached outputs, invalid unselected source cards, strict parsing, file protections, native state and metadata-only portable views. Platform generation is separate from installed-device validation.
