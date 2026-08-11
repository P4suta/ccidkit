# ADR-0004: exclusivity is spoken by types

- Status: accepted
- Date: 2026-08-11

## Context

A smart card is a single conversation: interleaving two APDU exchanges corrupts both,
and a select/response pair split by another process's command is the classic PC/SC bug —
the reason `SCardBeginTransaction` exists, and the reason every PC/SC tutorial warns
about it. The C API handles this with runtime brackets the caller must remember, paired
with a sharing violation error for when they do not.

Rust's ownership system can state the whole rule at compile time, but only if the API is
designed for it from the first signature: methods on `&self` with interior mutability
surrender the proof, and a generic associated type (GAT) borrow guard would infect every
trait bound its callers write.

## Decision

Every operation on every core trait takes `&mut self`. A transaction is
`Transaction<'c, C>`, a concrete guard type holding `&mut Card` — while it lives, no
other use of that card compiles, and dropping it ends the bracket. The concrete guard is
chosen over a GAT deliberately: the guard's shape is the same for every backend, so an
associated type would buy nothing and cost every signature that names it.
`wait_for_card` hands out a value whose drop releases the wait, so cancellation is
`drop` and cannot leak a parked thread.

There is no runtime lock inside the library, and no sharing-violation detection between
two handles of our own: the type system makes that state unrepresentable. (The OS-level
sharing violation from *other* processes remains real, and remains an error variant —
docs/adr/0012.)

The gate is mechanical, not cultural. The claim is proven by a compile-fail doctest
pair — one example that compiles because exclusivity is respected, one marked
`compile_fail` that differs only by the second borrow — run by `just test` on every
push, so the proof cannot rot into a comment.

## Consequences

APIs that want concurrent-looking access must say what they mean: monitoring a reader
while a card is in use belongs to `Monitor`, a separate object, not to a second handle
on the same card. Some ergonomic patterns (a card stored in a struct alongside things
that also borrow it) require restructuring, and that friction is the design working.

The `&mut` discipline also keeps every trait `Send`-agnostic (docs/adr/0011): nothing
here forces a mutex in, and nothing forbids the caller wrapping one around.

The doctest pair is specified before `Card` exists; it lands in the same commit as the
first `Transaction`, and this record is the reason a reviewer must not accept one
without the other.
