// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dev-only helpers for the workspace's own tests: byte builders, cassette fixtures,
//! and assertion utilities.
//!
//! This crate exists so test scaffolding shared between crates has a home that is
//! provably not a shipped artifact. It is `publish = false`, may be named only from a
//! dev-dependency table (`just deps`, rule R5), and declares no dependency of its own
//! (`ZERO_DEP`) so that using it never widens what a crate under test links.
//!
//! # Invariants
//!
//! - **Dev-only.** Reachable from `[dev-dependencies]` and nowhere else; the deps gate
//!   fails any normal edge to this crate.
//! - **Zero dependencies.** A helper that needs a third-party crate belongs in the test
//!   that needs it, not here.
//!
//! # Status
//!
//! Bootstrap: empty. First helpers arrive in M2 alongside the cassette schema
//! (docs/adr/0010, ROADMAP.md).
