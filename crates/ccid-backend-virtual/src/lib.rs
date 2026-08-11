// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The virtual backend: a scripted reader and card behind the `ccid-core` traits, for
//! tests that must run without hardware.
//!
//! This crate is a shipped product, not a dev fixture (docs/adr/0014). A downstream PIV
//! or eID implementation needs exactly what this workspace's own CI needs: a reader
//! that inserts, removes, answers, misbehaves, and hangs on schedule. Publishing it is
//! the difference between "our tests pass" and handing every adopter the means to say
//! the same.
//!
//! # Invariants
//!
//! - **A `Scenario` is a consuming builder**: insert cards, script exchanges, inject
//!   errors, and `hang()` — the step that never completes, which exists to prove
//!   `wait_for_card` cancellation stays drop-safe forever (docs/adr/0004, 0014).
//! - **Deterministic.** A scenario plays back the same way every run; timing knobs are
//!   part of the script, not the wall clock.
//! - **Workspace edges:** `ccid-core`, `ccid-apdu` (`just deps`). A future `vpcd`
//!   feature connects to a vsmartcard `vicc` over TCP so the same trait surface can
//!   front a full card OS emulation.
//!
//! # Status
//!
//! Bootstrap: no implementation. Version 1 lands in M3, where CI starts driving every
//! core trait through it on every push (ROADMAP.md).
