// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The CCID protocol, sans-I/O: message codec, class-descriptor interpretation, the
//! T=0 and T=1 machines, and the exchange planner.
//!
//! This crate exists so the protocol is testable without a device and reusable by any
//! transport. It encodes and decodes the CCID bulk messages (the `PC_to_RDR` and
//! `RDR_to_PC` families, with `bSeq` sequencing), interprets the class descriptor
//! (`dwFeatures`, exchange level, clock and voltage tables), and drives the card
//! protocols as data.
//!
//! # Invariants
//!
//! - **No I/O, no clock, no USB types** (docs/adr/0002). The machines emit actions and
//!   consume bytes; the transport owns time and retries. Waiting-time values (`Wt`) are
//!   advisory numbers this crate computes, never sleeps it performs.
//! - **T=0 and T=1 are enum state machines with `step` functions** (docs/adr/0008), not
//!   typestates: recovery transitions form a mesh, and a state that is data can be
//!   fuzzed, logged, and replayed from a cassette.
//! - **The `Exchanger` plans per exchange level** (docs/adr/0001): an APDU-level reader
//!   passes through, a TPDU-level reader runs the T machine, and a character-level
//!   reader is a stated day-one error rather than a silent misbehavior.
//! - **Quirks are a flag type** (`SlotQuirks`), set from the quirk table
//!   (docs/adr/0009), never inferred by sniffing mid-conversation.
//! - **The only dependency is `ccid-apdu`** (docs/adr/0013).
//!
//! # Status
//!
//! Bootstrap: frozen design, no implementation. The codec arrives in M2 against the
//! USB-IF CCID Revision 1.1 specification, the T machines in M4 with the vsmartcard
//! oracle (see ROADMAP.md).
