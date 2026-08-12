# ADR-0011: `!Send` is contained in workers

- Status: superseded by ADR-0016
- Date: 2026-08-11

## Context

PC/SC context and card handles are, in practice, thread-affine: `winscard` handles have
documented thread-use caveats, wrapper types over them are `!Send`, and blocking calls
like `SCardGetStatusChange` must be cancelable from another thread
(`SCardCancel`-style) without moving the handle. If that `!Send`-ness leaks into
`ccid-core`'s traits — a `Send` bound here, a "must be used on one thread" caveat
there — every backend and every caller inherits the platform's weakest threading model
forever. The native backend has no such affinity, and must not pay for the shim's.

At the same time the shim needs real concurrency: a `wait_for_card` that blocks in the
service must be cancelable by dropping the waiter (docs/adr/0004), and an unrelated
transmit on another reader must not queue behind a year-long wait.

## Decision

The shim contains its platform's threading model in workers. One worker thread per
PC/SC context owns every `!Send` handle of that context. Callers speak to it through a
`Job` enum over a channel; each job carries a oneshot reply channel. Cancellation is a
separate priority lane so a cancel never queues behind the blocking call it must
interrupt, and it is *drop-fired*: dropping the waiter sends the cancel, which is what
makes docs/adr/0004's drop semantics true here. `ccid-core` requires `Send` of nothing
and mentions threads nowhere.

`unsafe` follows the same containment logic at the crate level: whatever the FFI
boundary needs lives in `crates/ccid-backend-pcsc/src/` and nowhere else, each block
under a `// SAFETY:` comment. The workspace deliberately does not `forbid(unsafe_code)`
globally — the quarantine plus `unsafe_op_in_unsafe_fn = "deny"` is the policy, because
a forbid would just move the unsafe into a dependency where no gate of ours can see it.

The gate is mechanical, not cultural. `just unsafe-boundary` rejects the `unsafe` token
outside the quarantine and demands the SAFETY comment inside it; `just deps` keeps the
shim's crate edges narrow; and the virtual backend's `hang()` scenario (docs/adr/0014)
is the standing test that drop-fired cancellation actually cancels.

## Consequences

Every shim operation pays a channel round-trip. Against a stack whose operations are
milliseconds-to-seconds (card I/O), that cost is noise, and it buys a core API with no
threading footnotes.

The worker model is the one place docs/adr/0007's choice could collide with the `pcsc`
crate's ownership rules; if it does, that record's fallback activates, not a weakening
of this one.

The unsafe-boundary gate runs green today over a workspace with zero `unsafe` tokens,
so the quarantine exists before its first inhabitant and the first stray `unsafe`
fails its own commit.
