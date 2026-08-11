# ADR-0014: the virtual backend is a product

- Status: accepted
- Date: 2026-08-11

## Context

Every hardware-facing stack grows a mock for its own tests, and nearly every one keeps
it `publish = false`, which quietly decides that downstream users do not deserve the
test infrastructure the stack itself needed. The people building PIV, eID, or OpenPGP
card logic on top of `ccidkit` face the same problem this workspace faces in CI: no
reader attached, and behavior — insertion, removal, sharing, hangs — that must be
provoked on schedule to be tested at all.

There is also one specific promise that needs a permanent fixture: docs/adr/0004 makes
`wait_for_card` cancellation a drop, and docs/adr/0011 makes the shim's cancel lane
drop-fired. A promise like that rots unless something in the test suite waits on a
reader that will *never* answer and proves the drop still releases.

## Decision

`ccid-backend-virtual` is a published crate with API stability obligations, not a dev
fixture. `Scenario` is a consuming builder that scripts a reader's life: cards inserted
and removed, exchanges answered, errors injected (every variant of docs/adr/0012's
vocabulary must be scriptable), and `hang()` — the step that never completes.
`hang()` is not test sugar; it is the standing proof that waiting is cancel-safe, and
it stays in the public API forever. Scenarios are deterministic: identical runs,
timing expressed in script steps rather than wall-clock.

A future `vpcd` Cargo feature connects the same trait surface to a vsmartcard `vicc`
over TCP, so a full card OS emulation can sit behind the scripted reader without a new
backend.

The gate is mechanical, not cultural. `just deps` holds this crate inside the pure
layer (edges: `ccid-core`, `ccid-apdu` — enforced also by `just purity`, so the
scripted world stays dependency-free), and from M3 `just test-ci` drives every core
trait through this backend on every push, hardware absent by construction.

## Consequences

Publishing means the scenario API is designed, documented, and versioned rather than
accreted — real work that a `publish = false` mock would dodge. In exchange the
workspace's CI story and every adopter's CI story are the same story, and a bug
reproducible in a scenario is a bug anyone can rerun from a crates.io dependency.

vsmartcard (the `vpcd`/`vicc` pair) is verified live and unclaimed by any Rust bridge
(docs/M0-ground-truth.md), so the `vpcd` feature has a concrete oracle waiting in M4.

The crate is in the release group from day one; its `Scenario`/`hang()` surface is
frozen by this record before implementation, so M3 implements the contract rather than
discovering one.
