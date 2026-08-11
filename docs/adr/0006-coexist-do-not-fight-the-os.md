# ADR-0006: coexist; do not fight the OS

- Status: accepted
- Date: 2026-08-11

## Context

A CCID reader is one physical device, and on every mainstream platform some OS service
may already own it: `pcscd` on Linux when it is running, the Smart Card service on
Windows (which binds readers to its own driver), CryptoTokenKit/`PCSC.framework` on
macOS (which does not expose raw USB claiming for smart cards at all). A native USB
backend that grabs the interface anyway produces the worst possible developer
experience: works on the maintainer's machine, breaks login/eID/corporate VPN on the
user's, and the bug reports blame whichever side lost the race.

The temptation this ADR exists to refuse is "try native, fall back silently": silent
fallback means behavior differs between machines for invisible reasons, which is the
libccid failure mode this project measures itself against.

## Decision

Nothing in this workspace ever takes a device an OS service holds. The defaults, kept
as an explicit per-platform table in the facade:

- **Linux:** ccidkit's private native USB adapter. When claiming fails and `pcscd` plausibly
  holds the device, the error diagnoses the collision by name and says what to do —
  it never silently degrades.
- **Windows:** the `winscard` shim. Native access requires rebinding the reader to
  WinUSB, which is a driver change; it is supported as an explicit opt-in gesture
  (`ccdev` will assist), never a side effect of `open`.
- **macOS:** the `PCSC.framework` shim, permanently: the OS does not offer a
  supported raw path, and pretending otherwise would be fighting the OS by definition.

The gate is mechanical, not cultural. The default table is data with a unit test per
platform row (`just test`), so changing a platform's default is a visible diff to a
frozen table, and `just deps` keeps the facade the only crate that can express the
choice — no backend can unilaterally decide to fall back to another.

## Consequences

Linux users with `pcscd` running get an error where "grab it anyway" would sometimes
have worked; the error's diagnosis text is the feature. Windows native throughput
claims wait until a user has opted in to WinUSB; the shim is the first-run experience.
On macOS this project is a better API over the platform stack, full stop, and says so.

The M0 measurement makes this cheap to accept: the `pcsc` crate is 3 dependency crates
and platform-uniform, so keeping the shim first-class costs almost nothing
(docs/M0-ground-truth.md).

The default-table test is written with the table itself, before any backend can run, so
the first platform whose default drifts from this record fails its own commit.
