// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ccdev` — bring up, probe, and diagnose CCID readers.
//!
//! The maintainer-facing sibling of `ccid`: where the CLI proves the facade suffices,
//! this tool deliberately reaches below it — descriptors, quirk evaluation, exchange
//! traces — because diagnosing a reader means seeing what the facade hides. It is
//! `publish = false`: its audience is this repository and people filing quirk entries,
//! and its output format is allowed to change without a release.
//!
//! # Status
//!
//! Bootstrap: nothing is implemented. `ccdev doctor` — the ATR/`TA1`/IFSD/timeout-margin
//! verdict that turns a "reader does not work" report into a quirk-table entry with
//! evidence — lands in M5 (ROADMAP.md, docs/adr/0009).

fn main() {}
