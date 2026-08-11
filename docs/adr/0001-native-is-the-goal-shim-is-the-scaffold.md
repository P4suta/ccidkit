# ADR-0001: native is the goal; the shim is the scaffold

- Status: superseded by ADR-0016
- Date: 2026-08-11

## Context

Every program that talks to a smart card today goes through the platform PC/SC stack:
`pcscd` and the libccid driver on Linux, `winscard` on Windows, `PCSC.framework` on
macOS. The stack is C, daemon-shaped, and its developer experience is the `SCard*` API —
handles, reader name strings, manual transaction brackets. Rust wrappers exist, but they
wrap the shape rather than replacing it.

The M0 measurement (docs/M0-ground-truth.md) says the replacement is affordable: the
CCID driver's `src/` is 10,230 SLOC of C, and the part of it that is genuinely hard-won
knowledge — the per-reader hack logic — is 580 lines with 43 reader-specific case
labels. The bulk of libccid's value is declarative data: 652 supported-reader entries
and 700 descriptor dumps. Meanwhile `nusb` gives USB access with 6 dependency crates on
a Windows host and no C toolchain anywhere, and the `pcsc` crate covers the existing
stack with 3 crates.

Two products compete for the name "a Rust smart card stack": a better wrapper, or a
native driver. A wrapper can never fix the daemon's sharing semantics, its latency, or
its opacity when a reader misbehaves; a native driver cannot exist on platforms whose OS
refuses to hand over the device, and cannot be verified against real cards until it
works.

## Decision

The product is the Rust-native developer experience, and the native backend
(`ccid-backend-usb`, CCID over `nusb`) is the measuring stick every design choice is
held against. The PC/SC shim (`ccid-backend-pcsc`) exists for two reasons, both
permanent in intent but different in kind: it is the scaffold that lets every layer
above the backend be verified against real cards and real infrastructure before the
native backend lands, and it is the lasting coexistence route on platforms where the OS
does not release the device (docs/adr/0006).

Compatibility with the PC/SC C API is a non-goal. The shim consumes the service; nothing
in this workspace exports its shape.

The gate is mechanical, not cultural. `just deps` holds the shim to its narrow place in
the crate graph — `ccid-core` and `ccid-apdu`, never `ccid-proto` — so the scaffold
cannot quietly become the foundation, and `just test` runs the same trait suite over
every backend so the two routes cannot drift apart in behavior.

## Consequences

The upper layers must be written against traits that both a daemon-mediated and a
direct-USB backend can satisfy, which is a real constraint on `ccid-core` (docs/adr/0003,
0004) and is accepted deliberately.

In exchange: the project has a working, testable stack from M3 onward — `ccid list`,
`ccid atr`, `ccid apdu` over the shim — while the native backend is built with the hard
parts (T machines, quirks) arriving against an already-proven surface. The M0 numbers
above are the evidence the native half is a bounded effort rather than a rewrite of
pcsc-lite: the daemon (14,199 SLOC of C in `src/`) is exactly the part nothing here
replaces.

This gate and the deps matrix were written before any backend code exists, so the first
edge that would invert the design fails on the commit that introduces it.
