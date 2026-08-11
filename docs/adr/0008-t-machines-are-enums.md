# ADR-0008: the T machines are enums

- Status: accepted
- Date: 2026-08-11

## Context

T=0 and T=1 — the card-level protocols a TPDU-level reader leaves to the host — are
state machines with retransmission, resynchronization, chaining, and IFS negotiation.
Rust folklore says typestate: encode each protocol state as a type, make illegal
transitions unrepresentable. Typestate shines on APIs where the caller drives the
machine and the compiler should stop caller mistakes.

These machines are not that. They are internal to the stack (no caller ever holds one —
docs/adr/0002's `Exchanger` drives them), their recovery transitions form a mesh rather
than a ladder (almost any state can be interrupted by an R-block or an S-block request,
retried, or resynchronized), and their states need to be *data*: a fuzzer wants to
construct arbitrary mid-conversation states, a cassette replay (docs/adr/0010) wants to
serialize where the machine stood, and a `ccdev` trace wants to print it. Typestate
makes every one of those a fight, and the mesh of recovery arrows turns the type graph
into a combinatorial embarrassment.

## Decision

T=0 and T=1 are each one `enum` of states plus a `step` function:
`step(state, input) -> (state, actions)`. Transitions are `match` arms; illegal inputs
are handled arms returning protocol-error actions, not unrepresentable states. The
machines appear in no `ccid-core` trait — they are an implementation detail of
`ccid-proto` consumed by backends that face TPDU-level readers.

The gate is mechanical, not cultural. `just test` holds the transition tables to the
specification with table-driven cases and, from M4, the vsmartcard oracle; `just deps`
keeps the machines out of every crate but `ccid-proto`'s dependents that need them; and
the fuzz lane added in M4 feeds `step` arbitrary state/input pairs precisely because
states are constructible data.

## Consequences

The compiler no longer proves transition legality; the test table does. That trade is
accepted with eyes open: the states gain `serde`-ability (hex-in-TOML, docs/adr/0010),
fuzzability, and printability, which for an internal machine with mesh recovery is worth
more than a proof that only ever protected us from ourselves.

A later observation that some *caller-facing* protocol object would benefit from
typestate is not blocked by this record — this record is about the internal T machines
alone.

The transition-table tests are specified before the machines exist (M4), and the enum
shape is frozen now so the cassette schema (docs/adr/0010) can rely on states being
plain data.
