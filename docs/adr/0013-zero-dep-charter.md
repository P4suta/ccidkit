# ADR-0013: the zero-dependency charter

- Status: accepted
- Date: 2026-08-11

## Context

A stack that sits in login flows and signing tools is exactly where a security-conscious
adopter audits the dependency tree, and exactly where that tree tends to sprawl. The M0
measurement (docs/M0-ground-truth.md) established the budget the backends need: `nusb`
costs 6 dependency crates on a Windows host (38 across the all-platform union), `pcsc`
costs 3. Those are affordable *at the edges*. What must stay empty is the middle: the
vocabulary (`ccid-apdu`) that every downstream crate will type-share, the traits
(`ccid-core`) every backend implements, and the protocol (`ccid-proto`) that gets
fuzzed — none of them needs I/O, async runtimes, or serialization to do its job, and
each dependency any of them took would arrive transitively in every consumer forever.

The standing temptation is dev-dependencies: "just `proptest` for the unit tests". A
dev-dependency does not ship, but it does enter every audit, every `cargo deny` run,
and every build of the workspace — and a charter with a dev-shaped hole is not a
charter.

## Decision

`ccid-apdu` and `ccid-testkit` declare no dependencies at all, in any table, forever
(`ZERO_DEP`). The pure layer — `ccid-apdu`, `ccid-core`, `ccid-proto`,
`ccid-backend-virtual` — declares dependencies only on itself, in any table, dev
included, so nothing third-party arrives transitively anywhere inside it. The full
crate graph is the `ALLOWED` matrix in `xtask/src/deps.rs`: transitively closed,
self-referencing forbidden, every member covered (R7), normal edges only along its
arrows (R1), zero-dep crates empty (R2), dev edges never closing a cycle (R3), the
testkit reachable from dev tables alone (R5). The matrix is itself under test —
closure and no-self-reference are unit tests, so the checker's own premise is checked.

The gate is mechanical, not cultural. `just purity` and `just deps` run offline, in the
pre-commit hook and CI, and were written before any crate declares any dependency. The
no-`no_std`/no-wasm decision rides on this record too: the core is `std` (threads and
time live in backends' std anyway) yet I/O-free and dependency-free, so the purity gate
— not a target triple — is what carries the "runs anywhere Rust does" claim.

## Consequences

A test wanting `proptest` moves above the pure layer or into a fixture driven through
`ccid-testkit` (itself dependency-free); this bites, is known to bite, and is the cost
of a charter that cannot be talked around. Adding a crate to the workspace forces a
matrix row — a design decision in review, not a `Cargo.toml` drive-by.

The audit story becomes a sentence: the pure layer is this repository's own code, the
edges cost 3 and 6 crates respectively, measured, and `cargo deny` polices the rest.

Both gates and the matrix tests are green today over a workspace with zero declared
dependencies, so the first edge ever drawn is checked against a matrix that predates it.
