# ADR-0005: a status word is data, not an error

- Status: accepted
- Date: 2026-08-11

## Context

Every APDU response ends in a two-byte status word. `9000` is success; everything else
is a spectrum from "file not found" (`6A82`) through "security status not satisfied"
(`6982`) to warnings that carry data anyway (`62xx`). Libraries habitually turn non-9000
into an error, and it is the wrong altitude: whether `6A82` is a failure depends
entirely on what the caller was doing — probing for an applet by trying its AID *wants*
the `6A82` path to be ordinary control flow. An error type that encodes SW also tangles
retry logic: a transport failure is retryable, a card's considered answer is not.

Separately, the transport layer has genuinely mechanical response work: `61xx` (more
data available, fetch with GET RESPONSE), `6Cxx` (wrong Le, retry with the stated one),
extended length negotiation, and command chaining. Callers should not hand-write those
loops, but a diagnostic tool must be able to see the raw conversation.

## Decision

`transmit` returns `Ok(Response)` for every status word; `Err` is reserved for
transport and protocol failure — the exchange itself broke, not the card's answer. The
SW is data on `Response`: `sw.is_ok()`, `sw.meaning()`, and `response.require_ok()` for
the caller who wants `9000`-or-error, at the call site where that policy belongs.

`transmit` absorbs the mechanical dialogs — `61xx` continuation, `6Cxx` retry, extended
length, chaining — and returns the assembled response. `transmit_raw` sends exactly what
it was given and returns exactly what came back, for diagnostics and for the caller who
knows better.

The error vocabulary (docs/adr/0012) therefore contains no SW-shaped variant, and that
absence is load-bearing.

The gate is mechanical, not cultural. `just test` carries, from M2 on, the tests that
feed non-9000 responses through every backend and assert `Ok`; and `just lint` denies
the wall under which an SW-to-`Err` conversion helper would have to be written twice to
sneak past review once.

## Consequences

Callers who expect exceptions must write `require_ok()`, one call, visible. In exchange,
probing flows read as what they are, retry policy stays attached to genuinely retryable
variants (docs/adr/0012), and a backend can never disagree with another backend about
which SW values are "errors" — the question is unrepresentable below the caller.

`transmit`'s absorption means its byte trace differs from its argument; cassette capture
(docs/adr/0010) therefore records at the layer it captures, and `transmit_raw` exists
precisely so the absorbed dialogs remain observable.

The backend test suite that enforces this is specified before any backend exists;
`require_ok` and `transmit_raw` are part of the frozen surface, not later conveniences.
