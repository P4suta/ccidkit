# ADR-0003: AFIT and static dispatch; heterogeneity lives in one enum

- Status: superseded by ADR-0016
- Date: 2026-08-11

## Context

`ccid-core`'s traits are the surface every backend implements and every application
calls. Rust offers three ways to write them: `dyn Trait` objects (dynamic dispatch,
object-safety constraints, boxed returns), the `async-trait` crate (boxed futures on
every call), or plain traits with async fn in trait (AFIT) and static dispatch. The
choice is a one-way door: a trait designed for object safety gives up associated types
and `impl Trait` returns forever, and a facade built on `Box<dyn Backend>` bakes an
allocation and a vtable into every exchange.

Most applications use exactly one backend, chosen at compile time. The genuine need for
runtime heterogeneity is narrow: a tool like `ccid list` that shows every reader from
every backend at once, and a default table that picks per platform (docs/adr/0006).

## Decision

The core traits use AFIT and static dispatch. `ccid-core` contains no `dyn`, no
`async-trait` dependency, and no enum over backends. The one place runtime heterogeneity
exists is the facade: `ccidkit::Composite`, an enum whose variants hold the concrete
backends and whose trait impl delegates by hand-written `match`. Boring by design — the
match is the cost of heterogeneity, paid once, in the crate whose job is composition.

The gate is mechanical, not cultural. `just deps` keeps `async-trait` (and every other
third-party crate) out of `ccid-core` via the purity closure, and `just lint` denies the
workspace warning wall that a `dyn`-shaped shortcut would trip in review; the
compile-fail doctests of docs/adr/0004 depend on concrete types, so a `dyn` regression
breaks `just test` too.

## Consequences

Adding a backend means adding a `Composite` variant and its match arms — explicit,
slightly tedious, and impossible to get silently wrong. Downstream code written against
`impl Backend` monomorphizes; nothing pays for a vtable it does not use.

The constraint bites when someone wants a plugin system: dynamically loading a backend
at run time is out of scope, and this record is where that is stated. The facade enum is
the escape hatch that covers every known need short of that.

These rules were written before `ccid-core` holds a single trait, so the first
`async_trait` attribute or boxed future fails on the commit that introduces it.
