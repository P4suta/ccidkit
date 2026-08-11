# Roadmap

Each milestone is independently useful, and every milestone through M4 is verifiable by
`cargo test` alone — no reader attached. Hardware enters at M3 as an option and at M5 as
a subject.

## M0 — Measurement

Done, recorded in [docs/M0-ground-truth.md](docs/M0-ground-truth.md): the size of what
is being replaced (libccid `src/` ≈ 10.2k SLOC C; the per-reader hack logic 580 lines /
43 cases / 4 flags), the cost of the edges (`nusb` 6 crates on Windows, `pcsc` 3), the
availability of every crate name, and the liveness of the specification and the
vsmartcard oracle. The decisions those numbers forced are cited from the ADRs.

## M1 — Bootstrap freeze

This repository as it stands: ten crate skeletons, the frozen decision records
(docs/adr/0001..0015), and every gate — `purity`, `deps`, `unsafe-boundary`, `quirkdb`,
`bin-name`, the lint wall, REUSE — green in CI on all three operating systems before
any implementation exists.

## M2 — Vocabulary and codec

`ccid-apdu` (Command/Sw/Response/Atr/Aid) and `ccid-proto`'s message codec and class
descriptor interpretation, with ATR parsing under fuzz, the cassette schema
(docs/adr/0010) implemented in `ccid-testkit`, and the first cassettes committed.
"What does SW `6982` mean" is answerable from this milestone on, with zero
dependencies.

## M3 — Real cards through the shim

`ccid-core`'s traits, `ccid-backend-pcsc`'s worker model, `ccid-backend-virtual` v1,
and the facade's `Composite` and platform default table. Exit criteria: `ccid list`,
`ccid atr`, and `ccid apdu` work against a real card through the platform stack on all
three operating systems, and CI drives every core trait through the virtual backend —
including `hang()` cancellation — with no hardware in the loop.

## M4 — T machines and the Docker oracle

T=0/T=1 as enum machines in `ccid-proto` (docs/adr/0008), differentially tested against
vsmartcard's `vicc` in a Docker lane: the same exchanges through our machines and
through the emulated card must agree. The `vpcd` feature of the virtual backend lands
here.

## M5 — Native backend, quirk table v1, ccdev

`ccid-backend-usb` over `nusb`: enumeration, claiming, bulk/interrupt, the `Exchanger`
loop, hotplug. The quirk table earns its first real entries through `ccdev doctor` —
the ATR/`TA1`/IFSD/timeout-margin verdict that turns "my reader does not work" into an
evidenced `readers.toml` entry (docs/adr/0009). Linux native becomes the default it was
declared to be (docs/adr/0006).

## M6 — All platforms, 0.1 decision

Windows WinUSB opt-in rebinding assisted by `ccdev`; macOS confirmed permanently
shim-first; the release group cut at 0.1 once the facade's quickstart holds on all
three platforms. The 0.1 decision is a decision, not a merge side effect
(release-plz.toml says the same thing in configuration).

## Non-goals

PC/SC C API compatibility (the shim consumes the service; nothing exports its shape),
EMV payment stacks, and card OS implementations. The reader layer, done well, is the
whole ambition.
