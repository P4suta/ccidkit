## What this changes

<!-- One or two sentences. What behavior is different after this lands? -->

## Semantics

- [ ] `transmit` still returns `Ok(Response)` for any status word; nothing turned an SW
      into an `Err`, and nothing below `require_ok` decides what a "bad" SW is
- [ ] No backend type, `dyn`, or async-trait crate entered `ccid-core`; runtime
      heterogeneity stays in the facade's `Composite` enum
- [ ] `ccid-proto` still owns no I/O, no clock, and no USB type; time and retry execution
      stay with the transport

<!--
docs/adr/0002, 0003, 0005. The layering is the product; a shortcut through it is a bug
even when it works.
-->

## Safety

- [ ] No allocation is sized and no loop bounded by a number read out of a device's
      answer without checking it against the received byte count first
- [ ] Arithmetic on device-derived lengths states its overflow behavior; no unchecked
      indexing or slicing
- [ ] Any `unsafe` stays inside `crates/ccid-backend-pcsc/src/` with a `// SAFETY:`
      comment (`just unsafe-boundary`)

## Quirks

- [ ] Any new `quirks/readers.toml` entry carries a reproduction of our own — a
      cassette, an issue, or a capture — not a transcription from libccid's Info.plist
- [ ] Any new flag was added to the vocabulary in `xtask/src/quirkdb.rs` with a sentence
      saying what it means, in this same change

## Checks

- [ ] `just ci` passes locally
- [ ] The crate graph still matches the ALLOWED matrix and the pure layer took nothing
      from outside (`just deps`, `just purity`)
- [ ] No `allow` or `ignore` was added to make a gate pass

<!--
If a gate was changed rather than satisfied, say why here. That is sometimes right, and
it always deserves a sentence. The legitimate route is the shared configuration —
clippy.toml, the workspace lints table, deny.toml — never a local suppression. See
CONTRIBUTING.md.
-->
