# Roadmap

The architectural baseline is implemented. The remaining work is evidence and release
hardening, not another public abstraction layer.

## Implemented

- one published `ccidkit` library and a binary-only consumer surface;
- validated short/extended APDU, response/status, and ATR values;
- runtime-neutral wait/await `Operation` with drop cancellation;
- `Context → Reader → Card → Transaction` ownership facade;
- per-card FIFO worker and real PC/SC transaction brackets;
- platform PC/SC adapter and native USB CCID descriptor/bulk transport;
- pure CCID codec and T=1 block/action machine for TPDU readers;
- scripted virtual reader, hot-plug monitor vocabulary, and bounded trace stream;
- `ccid` quick-path CLI and `ccdev doctor` diagnostics;
- architecture, dependency, purity, no-unsafe, quirk, lint, test, and targeted mutation
  gates.

## Before 0.1

1. Run the hardware conformance matrix across representative T=0, T=1, APDU-level,
   TPDU-level, multi-slot, contact, and contactless readers on Linux/Windows/macOS; use
   it to admit native T=0 TPDU and T=1/CRC only after differential evidence exists.
2. Add only reproduced device quirks, each with a capture or issue receipt.
3. Differentially exercise the T=1 machine against vsmartcard and preserve the
   conversations as deterministic tests.
4. Measure card-removal and timeout behavior on each backend; tune bounded waits from
   evidence rather than reader folklore.
5. Run the full release feature/MSRV/documentation/mutation gates and decide whether the
   compact public surface is ready to carry `0.1` compatibility.

## Explicit non-goals

PC/SC C API compatibility, third-party backend plugins, EMV/payment application stacks,
card operating systems, runtime-specific APIs, and character-level CCID readers.
