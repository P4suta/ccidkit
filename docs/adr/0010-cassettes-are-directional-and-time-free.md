# ADR-0010: cassettes are directional and time-free

- Status: superseded by ADR-0016
- Date: 2026-08-11

## Context

Testing a protocol stack against hardware that is not attached requires recorded
conversations — cassettes. Two capture sources exist at two different layers: `pcsc-spy`
logs record APDU-level exchanges (what an application said to a card through the
platform stack), and usbmon/Wireshark `pcapng` captures record CCID-level bulk traffic
(what a driver said to a reader). Both are needed: APDU cassettes verify the upper
stack and card-facing logic; CCID cassettes verify the codec and the exchange loop
against real reader behavior, and they are the evidence format the quirk table
(docs/adr/0009) most wants.

Raw captures make poor fixtures: they carry timestamps that make diffs noisy and
replays timing-dependent, and their framing is capture-tool-specific. A fixture format
must be reviewable in a pull request — a human should be able to read the conversation.

## Decision

One cassette schema, TOML, with two declared flavors: `layer = "apdu"` (typically
distilled from pcsc-spy) and `layer = "ccid"` (typically distilled from usbmon/pcapng).
Every record is explicit about direction — `PcToRdr` or `RdrToPc` — and bodies are hex
strings (hex serde), so a cassette diffs and reviews like prose. Cassettes carry **no
timestamps**: replay asserts order and content, never latency, because a test that
depends on recorded timing is flaky by construction. Where a waiting-time matters it is
an asserted *action* of the T machine (docs/adr/0002), not a recorded gap.

The gate is mechanical, not cultural. The cassette loader in `ccid-testkit` (M2)
rejects records without a direction and any timestamp-shaped field by schema, and
`just test` replays every committed cassette on every push; `typos` and review stay
useful because the format is text.

## Consequences

Distilling a capture into a cassette is a manual, lossy step — deliberately: the
distillation is where a maintainer decides what the fixture asserts. Raw captures may be
kept as quirk-table evidence, but tests run from cassettes only.

Time-free replay cannot reproduce timing-triggered bugs; those get targeted tests with
explicit injected clocks at the transport layer, not cassette reruns.

The schema is frozen here, before the loader exists, so the first cassette committed in
M2 is already in the final format and no fixture migration ever needs to happen.
