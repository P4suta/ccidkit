# ADR-0016: one library, one operation type, private implementation architecture

- Status: accepted
- Date: 2026-08-11
- Supersedes: ADR-0001, 0002, 0003, 0007, 0008, 0010, 0011, 0012, 0013, 0014,
  and 0015

## Context

The bootstrap split protocol vocabulary, traits, three backends, a testkit, and a
facade into seven prospective public libraries before any user needed those seams.
Every crate made an implementation choice independently versioned, discoverable, and
effectively permanent. The trait layer also forced the design to solve third-party
backend extensibility, despite no product requirement for plugins.

The desired product is the opposite: an exceptionally small Rust contract that can be
used synchronously or asynchronously, while keeping the native stack internally
testable. C and C++ libraries show the cost of leaking transport handles, backend
vocabularies, and broad extension surfaces into decades of compatibility work.

## Decision

`ccidkit` is the only published library. APDU/ATR values, pure protocol machines,
workers, and built-in backends are private modules. `ccid` is a binary package and
`ccdev` is unpublished tooling. Internal purity is enforced against source modules;
crate boundaries are not used as a substitute for architecture.

The public effect is one concrete `Operation<'a, T>` which implements `Future` and also
offers blocking waits. The ownership path is concrete: `Context`, `Reader`, `Card`, and
the `Transaction<'_>` borrow guard. Runtime heterogeneity is a private enum. There is no
public backend trait or SPI.

Errors expose a small non-exhaustive portable category and an opaque message. The
opaque value may retain a private source for diagnostics without exposing dependency
types. The repository forbids unsafe code entirely; safe dependency APIs own their FFI
and OS invariants.

The virtual reader remains a product feature as `ccidkit::testing`, using a finite
consuming scenario vocabulary rather than callbacks or an implementation trait.
Time-free scripted exchanges replace a separately versioned cassette crate/schema.

## Consequences

One release carries one library compatibility promise. Refactoring a worker, replacing
`nusb`/`pcsc`, changing protocol state representation, or merging modules is invisible
downstream. The public API cannot host arbitrary third-party backends; that is a chosen
non-goal and may only be revisited with concrete users and a separate compatibility
surface.

Internal boundaries need mechanical support because Cargo no longer supplies them.
`just purity` checks that the pure files cannot name effects or adapters, `just deps`
checks the tiny workspace graph, and `just mutants` tests the semantic force of parser
and state-machine assertions. Compile-fail documentation proves transaction borrowing.

The platform adapter dependencies still vary by target, but neither their types nor
their versions affect users of the facade. This is the intended cost boundary.
