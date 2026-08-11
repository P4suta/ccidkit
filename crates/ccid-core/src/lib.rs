// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The traits every backend implements and every application calls: `Backend`, `Reader`,
//! `Card`, `Transaction`, and `Monitor`, plus the error vocabulary and `ReaderInfo`.
//!
//! This crate exists so the application-facing surface is written once, above every
//! transport. A PIV tool written against these traits runs unchanged over native USB
//! CCID, over the platform PC/SC service, and over the virtual backend in CI.
//!
//! # Invariants
//!
//! - **Exclusivity is spoken by types** (docs/adr/0004). Every operation takes
//!   `&mut self`, and a transaction is a concrete guard borrowing `&mut Card` — while it
//!   lives, no other use of the card compiles. There is no runtime lock to forget.
//! - **Static dispatch, native `async`-free traits** (docs/adr/0003). No `dyn`, no boxed
//!   futures, no backend enum in here; runtime heterogeneity belongs to the facade's
//!   `Composite` type and nowhere else.
//! - **A status word is data** (docs/adr/0005). `transmit` returns `Ok(Response)` even
//!   when the card answers `6A82`; `Err` is reserved for transport and protocol failure.
//!   `transmit` absorbs `61xx`/`6Cxx` continuation, extended length, and chaining;
//!   `transmit_raw` sends exactly what it was given.
//! - **Errors are one flat list** (docs/adr/0012): `NoReader`, `CardAbsent`, `CardGone`,
//!   `SharingViolation`, `Timeout`, `Transport`, `Protocol`, `NotSupported` —
//!   `non_exhaustive`, no source chain, and the retryable conditions are their own
//!   variants rather than a flag.
//! - **Waiting is cancel-safe.** Dropping the value `wait_for_card` hands out releases
//!   the wait; the virtual backend's `hang()` exists to prove that forever.
//! - **The only dependency is `ccid-apdu`** (docs/adr/0013), and nothing third-party
//!   arrives transitively.
//!
//! # Status
//!
//! Bootstrap: the trait set and error vocabulary above are frozen in docs/adr/ and not
//! yet written. First implementation lands in M2..M3 (see ROADMAP.md); the compile-fail
//! doctest pair proving the transaction guard (docs/adr/0004) lands with it.
