// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shim backend: the platform PC/SC service (`winscard` on Windows,
//! `PCSC.framework` on macOS, `pcsc-lite` on Linux) behind the `ccid-core` traits.
//!
//! This crate exists twice over (docs/adr/0001): as the scaffold that lets every upper
//! layer be verified against real cards before the native backend exists, and as the
//! permanent coexistence route on platforms whose OS will not hand over the device
//! (docs/adr/0006). One crate covers all three platforms through the `pcsc` crate
//! (docs/adr/0007) — a hand-written two-crate FFI split is the recorded fallback, not
//! the plan.
//!
//! # Invariants
//!
//! - **This is the workspace's one unsafe quarantine** (docs/adr/0011). Any `unsafe`
//!   the FFI boundary demands lives here and nowhere else, each block under a
//!   `// SAFETY:` comment; `just unsafe-boundary` enforces both directions.
//! - **`!Send` is contained in workers** (docs/adr/0011): one worker thread per PC/SC
//!   context, a `Job` enum over a channel, oneshot replies, a priority lane for
//!   cancellation, and drop-fired cancel — so `ccid-core` never has to demand `Send`
//!   from anyone.
//! - **PC/SC C API compatibility is a non-goal.** The shim consumes the service; it
//!   does not re-export its shape.
//! - **Workspace edges:** `ccid-core`, `ccid-apdu` (`just deps`). The T machines are
//!   not needed here — the service speaks APDU.
//!
//! # Status
//!
//! Bootstrap: no implementation. The worker model and the first three verbs
//! (list, connect, transmit) land in M3 (ROADMAP.md).
