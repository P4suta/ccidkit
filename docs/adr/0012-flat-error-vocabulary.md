# ADR-0012: a flat error vocabulary

- Status: accepted
- Date: 2026-08-11

## Context

Error design decides what callers can do at 2 a.m. The failure modes of a smart card
stack are few and behaviorally distinct: nothing to talk to, the card left, someone
else holds it, it took too long, the pipe broke, the protocol broke, the operation is
not supported here. Two of them — the card vanished mid-conversation, and another
process holds the reader — are *retryable by a human gesture* (reinsert the card, close
the other application) and callers genuinely branch on them.

The failure modes this vocabulary must *not* absorb: status words (a card's answer is
data, docs/adr/0005), and deep source chains (`Transport(io::Error(Os { code: 995
... }))`) that force callers to pattern-match through wrapper layers that differ per
backend — the classic way "one API over several backends" leaks which backend you are
on.

## Decision

One flat `Error` enum in `ccid-core`: `NoReader`, `CardAbsent`, `CardGone`,
`SharingViolation`, `Timeout`, `Transport`, `Protocol`, `NotSupported`. Marked
`non_exhaustive` so a variant can be added without a major version. No `source()`
chain: each variant carries its own display-ready context (backends translate their
internals into it), so matching a variant is the whole story and two backends failing
the same way produce the same value. The retryable conditions are their own variants —
`CardGone` and `SharingViolation` are never folded into `Transport` — because "retry
after human gesture" is the branch callers write. No variant contains an SW.

The gate is mechanical, not cultural. `just test` carries the cross-backend error
mapping suite from M3 — the same provoked failure must yield the same variant from the
shim and the virtual backend — and `just lint`'s `missing_docs` wall forces every
variant added later to say when a caller receives it.

## Consequences

Dropping `source()` costs debug depth, and the compensation is deliberate: backends log
their internal cause at capture point (and `ccdev` shows raw traces), rather than
exporting it through the type. Adding context to a variant later is an additive change;
`non_exhaustive` keeps downstream matches compiling through vocabulary growth.

The virtual backend must be able to *script* every variant (docs/adr/0014), which is
how the mapping suite exists without hardware.

The vocabulary is frozen here, before any backend produces a single error, so the
mapping suite tests against this record rather than against whichever backend landed
first.
