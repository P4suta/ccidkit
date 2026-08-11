# ADR-0009: quirks are data with provenance

- Status: accepted
- Date: 2026-08-11

## Context

Reader misbehavior is the moat around every CCID implementation — or so the folklore
says. The M0 measurement (docs/M0-ground-truth.md) cut the moat down to size: libccid's
entire per-reader hack logic is 580 lines of C — `ccid_open_hack_pre` (129 lines, 16
case labels), `ccid_open_hack_post` (349 lines, 27 case labels), and the Gemalto
firmware helpers between them (95 lines) — steered by just 4 named `DRIVER_OPTION`
flags plus a voltage-sequence bit pair. The bulk of its reader knowledge is data:
`supported_readers.txt` (652 unique VID:PID:name entries), 700 descriptor dump files,
and 5 pinpad feature-override files in `readers/extra_features/` — a second, tiny quirk
channel worth modeling in the same table.

Data wants to be a table. The trap is *how the table gets filled*: bulk-transcribing
libccid's Info.plist would import 652 conclusions without their evidence, in a workspace
whose license (MIT/Apache-2.0) does not admit copying from libccid anyway, and every
transcribed entry would be a claim nobody here can defend when it turns out wrong.

## Decision

Reader quirks live in `quirks/readers.toml`: one entry per model — `vid`, `pid`,
`name`, `flags[]` from a closed vocabulary, and a `provenance` block (`source`,
`evidence`, `date`) naming the reproduction that earned the entry: a cassette, an
issue, or a capture. An entry without our own reproduction does not go in. libccid's
Info.plist is a *place to check a suspicion* — reading facts is fine — never a source to
transcribe in bulk.

The table starts empty and the pinpad-override channel is a flag in the same table, not
a second mechanism.

The gate is mechanical, not cultural. `just quirkdb` enforces the schema — ascending
unique (vid, pid), full provenance, flags in vocabulary — and runs green over zero
entries so the rules bind from the first entry. The vocabulary lives in
`xtask/src/quirkdb.rs` and grows only in the same change that first uses a flag.

## Consequences

This table will grow slowly, and that is the accepted cost: coverage claims stay honest
because every claim carries its receipt. The 652-entry head start is deliberately left
on the table; what is taken instead is the *shape* lesson — quirks are data, and 4
flags plus a voltage option covered 20 years of hardware, so the vocabulary starts
small and concrete.

`ccdev doctor` (M5) exists to make earning an entry cheap: it turns a "reader does not
work" report into the evidence block this schema demands, and the reader-report issue
template asks for exactly the same fields.

The quirkdb gate is written and green before the first entry exists, so the schema is
enforced from entry one rather than retrofitted over folklore.
