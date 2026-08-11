// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ccid` — list readers, read ATRs, and exchange APDUs from the command line.
//!
//! A thin front end over `ccidkit`, and deliberately only that: the CLI depends on the
//! facade alone (`just deps`), so every capability it has is proof the facade's public
//! surface suffices for a real program.
//!
//! # Status
//!
//! Bootstrap: no subcommand exists. `ccid list`, `ccid atr`, and `ccid apdu` land in M3
//! against the shim backend (ROADMAP.md).

fn main() {}
