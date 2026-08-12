# Architecture

The architecture optimizes for the smallest durable contract, not the largest reusable
crate graph. Reasoning is recorded in [`docs/adr/`](docs/adr/); this file states the
current invariants.

## Shape

```text
 applications / ccid / ccdev
             │
       public ccidkit API
 Context → Reader → Card → Transaction
             │
      ordered card actor ───── bounded diagnostics
       ┌─────┼──────┐
   native   PC/SC   virtual          I/O adapters (private)
      │
 CCID transport driver
      │
 model · CCID codec · T=1 machine    pure transformations (private)
```

Only `ccidkit` is a published library. Private modules are real architectural
boundaries, enforced by source-level gates and review, without turning implementation
choices into crates.io packages or semver promises.

## Public contract

The public surface has four groups:

| Group | Types |
| --- | --- |
| entry and ownership | `Context`, `Reader`, `Card`, `Transaction`, `Monitor` |
| values | `Command`, `Response`, `StatusWord`, `Atr`, `ReaderId`, `Capabilities` |
| effects | `Operation<T>`, `Error`, `ErrorKind`, `Result<T>` |
| opt-in support | `testing::Scenario`, trace values, `BackendKind` |

There are no public backend traits, backend handles, descriptor types, extension
points, or dependency types. Adding a built-in backend changes private dispatch, not
every downstream implementation.

## Effect model

`Operation<'a, T>` is both a `Future<Output = Result<T>>` and a blocking handle. It is
implemented with a completion cell, condition variable, waker, and cancellation flag;
it does not embed an executor.

Every connected card owns one worker thread and FIFO job channel. That actor is the
only owner of the transport handle and therefore the single serialization point. A
dropped operation requests cancellation:

- an observation that has not produced a side effect stops promptly;
- a card command already submitted is drained before the next job;
- a scripted `hang()` observes cancellation and releases the worker;
- no second command can be constructed from the same `Card` borrow meanwhile.

`Transaction<'_>` holds `&mut Card`. The worker enters a nested service loop for the
guard's lifetime; PC/SC holds its OS transaction object in that loop, while native and
virtual cards rely on the same exclusive actor. Dropping the guard enqueues the end
marker.

## Protocol policy

- Parsers accept complete, validated frames and reject truncation, trailing bytes,
  reserved statuses, stale sequence numbers, and overflow.
- `Command` preserves an explicitly extended APDU encoding even when the semantic
  length could also be represented in short form.
- `Response` contains every status word. `require_success()` is caller policy.
- `transmit()` handles `6Cxx` once and `61xx` chaining with a bounded loop;
  `transmit_raw()` never invents another exchange.
- The CCID codec and T=1 machine contain no I/O, clock, worker, USB, or PC/SC import.
  They consume bytes/state and emit bytes/actions.
- CCID time extensions, APDU chaining, and T=1 action counts are bounded so a hostile
  device cannot create an unbounded internal loop.

## Backend policy

`BackendKind` is selection data, not an implementation interface. Platform defaults
coexist with the operating system: Linux attempts native USB and reports `Busy` when
another service owns the interface; Windows/macOS use their PC/SC services. Windows
native USB is opt-in because it requires the device to be bound to WinUSB.

The virtual backend is part of the public developer experience but not a separate
crate. Its consuming `Scenario` vocabulary is intentionally finite; arbitrary callback
hooks would become a second backend SPI.

## Diagnostics and errors

Portable error categories are a flat, non-exhaustive enum. `Error` is opaque and may
retain a private source for debugging; backend error enums never escape. Retryability
is computed from the portable category.

Tracing is opt-in because APDUs may carry secrets. Each subscriber has a bounded queue;
overflow is an explicit event instead of hidden memory growth or silent loss.

## Mechanical gates

| Gate | Enforces |
| --- | --- |
| `just deps` | binaries depend only on `ccidkit`; workspace graph is fully listed |
| `just purity` | model/CCID/T=1 modules import no effects or backends |
| `just unsafe-boundary` | no repository source contains an unsafe block/function |
| `just quirkdb` | quirks are sorted, unique, vocabulary-checked, and evidenced |
| `just bin-name` | binaries and library crates never share an artifact name |
| `just lint` | strict arithmetic/indexing/documentation wall across all features |
| `just test` | behavior, cancellation, and ownership compile-fail proofs |
| `just mutants` | critical parser and state-machine comparisons are test-sensitive |

The only published-library decision and its consequences are frozen in
[ADR-0016](docs/adr/0016-one-library-one-operation.md).
