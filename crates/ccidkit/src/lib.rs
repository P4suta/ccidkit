// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The crate to depend on: the whole stack behind one door.
//!
//! This facade re-exports the vocabulary (`ccid-apdu`), the traits (`ccid-core`), and
//! the backends, and adds the three things that belong above all of them:
//!
//! - **`Composite`**, the one enum over the backends, delegating by hand-written
//!   `match` (docs/adr/0003). The core traits stay statically dispatched; a program
//!   that must pick its backend at run time picks here and nowhere lower.
//! - **The platform default table** (docs/adr/0006): Linux prefers native USB and
//!   diagnoses a `pcscd` collision by name; Windows defaults to the `winscard` shim
//!   with WinUSB rebinding as an explicit opt-in; macOS defaults to the
//!   `PCSC.framework` shim. Coexistence over conquest, spelled out in one table.
//! - **The quickstart surface**: connect to the first card, transmit, done — for the
//!   caller who wants a card, not a stack.
//!
//! This crate is also the workspace's changelog carrier: release notes for every crate
//! in the version group live in this repository's CHANGELOG.md.
//!
//! # Invariants
//!
//! - Everything reachable from here obeys the ALLOWED matrix (`just deps`); the facade
//!   is the widest point and still names no third-party crate of its own.
//! - The binary named `ccid` lives in `ccid-cli`, never here: a facade and a binary
//!   sharing a name fight over `target/doc` (docs/adr/0015, `just bin-name`).
//!
//! # Status
//!
//! Bootstrap: no implementation. Re-exports and `Composite` arrive with the first
//! backend in M3 (ROADMAP.md).
