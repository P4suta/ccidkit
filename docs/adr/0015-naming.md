# ADR-0015: naming

- Status: superseded by ADR-0016
- Date: 2026-08-11

## Context

Names in a multi-crate workspace are one-way doors: crates.io names are permanent,
binary names end up in shell histories and scripts, and a facade crate sharing a name
with a binary target makes parallel rustdoc fight over `target/doc/<name>` until
`cargo doc` dies — a failure this workspace family has hit before and now designs
against. The M0 measurement (docs/M0-ground-truth.md) confirmed the whole namespace is
open: all ten planned names return 404 on crates.io, and bare `ccid` is unclaimed too.

## Decision

- The facade is **`ccidkit`** and is the crate to depend on.
- Every library crate is prefixed **`ccid-`**: `ccid-apdu`, `ccid-core`, `ccid-proto`,
  `ccid-testkit`, `ccid-backend-usb`, `ccid-backend-pcsc`, `ccid-backend-virtual`,
  `ccid-cli`, `ccid-driverkit`.
- The binaries are **`ccid`** (in `ccid-cli`) and **`ccdev`** (in `ccid-driverkit`).
  A bin target's name must differ from every lib crate's name; `ccid` and `ccdev`
  collide with nothing above.
- The bare crates.io name `ccid` stays unclaimed by us: squatting it for a stub
  contradicts what this project would want done unto it.

The gate is mechanical, not cultural. `just bin-name` derives the lib and bin name sets
from the manifests — implicit `src/main.rs` bins included — and fails any overlap, so
the rustdoc collision is unrepresentable in a green tree.

## Consequences

`ccid list` reads as the tool it is, `ccidkit` reads as the library it is, and the two
never contend for a documentation path. The `ccid-` prefix makes workspace membership
legible in a downstream `Cargo.lock` at a glance.

The cost is that the CLI's package name (`ccid-cli`) differs from its binary (`ccid`),
which surprises exactly once and is documented in the crate's own manifest comment.

The bin-name gate is green today, before any binary does anything, so a future target
rename that would collide fails on its own commit rather than in a release week's
`cargo doc` run.
