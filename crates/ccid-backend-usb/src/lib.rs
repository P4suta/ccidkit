// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The native backend: CCID over USB, directly, with no daemon in between.
//!
//! This crate is the destination the whole workspace aims at (docs/adr/0001): it
//! enumerates interface class `0x0B` devices, claims the interface, moves the bulk and
//! interrupt endpoints, and drives `ccid-proto`'s `Exchanger` — supplying the time,
//! retries, and transfer sizing the sans-I/O layer deliberately does not own
//! (docs/adr/0002). It applies the quirk table (docs/adr/0009) at open, and surfaces
//! hotplug through the core `Monitor` trait.
//!
//! # Invariants
//!
//! - **Pure Rust all the way down.** The USB transport is `nusb`; no C toolchain and no
//!   system smart card stack is required or touched.
//! - **Coexistence, not conquest** (docs/adr/0006). This backend never steals a device
//!   an OS service holds: on Linux it diagnoses a `pcscd` collision by name; on Windows
//!   a WinUSB rebind is an explicit opt-in gesture, never a side effect of `open`.
//! - **Workspace edges:** `ccid-core`, `ccid-apdu`, `ccid-proto`, and nothing else
//!   (`just deps`).
//!
//! # Status
//!
//! Bootstrap: no implementation. Enumeration and the first exchange land in M5, after
//! the shim has proven the upper layers against real cards (ROADMAP.md).
