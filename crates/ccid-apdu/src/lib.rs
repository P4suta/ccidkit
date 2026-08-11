// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The vocabulary of the card interface: command and response APDU, status word, ATR,
//! and AID.
//!
//! This crate exists so that every layer above it — the traits in `ccid-core`, the wire
//! codec in `ccid-proto`, every backend, and every downstream PIV or eID implementation —
//! speaks about cards in one set of types. It holds `Command` (a consuming builder, so a
//! half-configured command cannot escape), `Le`, `Sw` (with `is_ok` and `meaning`),
//! `Response` (with `require_ok`), `Atr` (`TS`, `T0`, the `TA1`..`TDi` interface bytes,
//! historical bytes, `TCK`), and `Aid`.
//!
//! # Invariants
//!
//! - **Zero dependencies, forever.** This is the workspace's dependency-zero charter
//!   (docs/adr/0013): no third-party crate in any dependency table, dev-dependencies
//!   included, enforced by `just purity` and `just deps`.
//! - **No I/O.** Everything here is a pure function over bytes the caller already holds.
//! - **A status word is data, not an error** (docs/adr/0005). `Sw` values other than
//!   `9000` construct fine, compare fine, and explain themselves through `meaning`;
//!   turning one into a failure is the caller's explicit `require_ok` call.
//! - **Parsing refuses rather than guesses.** An ATR whose checksum fails or whose
//!   interface-byte chain overruns the buffer is an error naming the offset, never a
//!   silently truncated value.
//!
//! # Status
//!
//! Bootstrap: the surface above is the day-one frozen design (docs/adr/), and none of it
//! is implemented yet. This crate gains code in M2 (see ROADMAP.md) and its API is fixed
//! by the decision records before the first implementation commit.
