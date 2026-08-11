# ADR-0002: the protocol core is sans-I/O

- Status: accepted
- Date: 2026-08-11

## Context

The CCID protocol is a request/response conversation over bulk endpoints with an
interrupt side channel, and the card protocols beneath it (T=0, T=1) are byte-level
state machines with timing rules. The obvious implementation entangles all of that with
the USB library: the codec reads from an endpoint, the T machine sleeps for a waiting
time, the retry loop calls the transfer function. Entangled protocol code cannot be
tested without hardware, cannot be reused by a different transport (the shim, the
virtual backend, a future vpcd bridge), and cannot be fuzzed effectively because every
fuzz input drags a transport behind it.

The waiting-time rules are the tempting exception: `WT` and `BWT` feel like they belong
next to the state machine that computes them. But the moment the machine owns a clock it
owns a runtime, and the crate stops being pure.

## Decision

`ccid-proto` holds no I/O, no clock, and no USB types. The message codec transforms
bytes; the T=0/T=1 machines are data (docs/adr/0008) whose `step` functions consume
input and emit actions — "send these bytes", "expect a block", "a waiting time of this
many ticks applies" — and the transport executes actions, owns time, and performs
retries. Waiting-time values are advisory numbers this crate computes, never sleeps it
performs. The `Exchanger` plans an exchange for a given reader level and leaves the
driving to the caller.

The gate is mechanical, not cultural. `just purity` and `just deps` refuse `ccid-proto`
every dependency except `ccid-apdu`, which is what makes "no USB types" checkable: the
crate cannot name a type it cannot import. `just lint` holds the arithmetic and
indexing rules that keep the codec honest on device-supplied lengths.

## Consequences

Backends carry more code than they would with an entangled core: each one drives the
`Exchanger` loop itself. That duplication is shallow — a loop per transport — while the
duplication this avoids is deep: two implementations of T=1 resequencing, one for USB
and one for the vpcd bridge, drifting apart.

Every protocol test in M2 and M4 runs from byte fixtures and cassettes without
hardware; the fuzzer feeds the codec and the machines directly. The vsmartcard oracle
(M4) exercises the same machines over TCP that the USB backend exercises over bulk
transfers, which is the point.

The purity and deps gates were written before the crate holds a line of protocol code,
so the first `std::time` or `nusb` import inside `ccid-proto` fails on its own commit.
