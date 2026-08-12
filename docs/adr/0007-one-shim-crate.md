# ADR-0007: one shim crate

- Status: superseded by ADR-0016
- Date: 2026-08-11

## Context

The shim must speak to three different system stacks: `winscard.dll`, `pcsc-lite`, and
`PCSC.framework`. Two shapes were on the table. First, hand-written FFI split into a
`-sys` declarations crate and a safe wrapper — maximum control, and the shape the
workspace's unsafe-quarantine tooling (docs/adr/0011) is built to police. Second, the
existing `pcsc` crate, which already wraps all three platforms behind one safe API.

The M0 measurement (docs/M0-ground-truth.md) settled the cost question: `pcsc` v2.9.0
brings exactly 3 dependency crates (`pcsc`, `pcsc-sys`, `bitflags`), identical on every
platform — it links the system stack rather than replacing it, which is precisely the
shim's job description. Hand-written FFI would re-earn that coverage one platform quirk
at a time, in the one crate whose entire purpose is to be scaffolding (docs/adr/0001).

The genuine risk is the worker model: the shim contains `!Send` context state and
drop-fired cancellation (docs/adr/0011), and a third-party wrapper's ownership model
might not compose with a per-context worker thread.

## Decision

`ccid-backend-pcsc` is one crate covering all three platforms through the `pcsc` crate.
The hand-written two-crate FFI split is the recorded fallback, to be taken only if the
worker model and the `pcsc` crate's ownership rules genuinely collide — and if that day
comes, this record is superseded rather than silently contradicted, and the quarantine
tooling is already shaped for it.

The gate is mechanical, not cultural. `just deps` pins the shim's workspace edges
(`ccid-core`, `ccid-apdu`), `just purity` keeps its third-party surface out of the pure
layer, and `just unsafe-boundary` polices whatever `unsafe` the boundary needs
regardless of which FFI route is under it.

## Consequences

The shim inherits the `pcsc` crate's release cadence and its abstraction choices; where
those chafe, the response is a patch upstream or the recorded fallback, not a partial
in-tree fork. Three platforms cost one `cfg`-light codebase, and the shim stays thin
enough that deleting it from a dependency tree (a native-only embedded user) removes the
whole PC/SC world.

The quarantine and deps gates were in place before this crate's first line, so the
fallback — if ever taken — starts from enforced boundaries rather than retrofitting
them.
