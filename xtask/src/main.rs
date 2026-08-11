// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Repository maintenance tasks for the ccidkit workspace.
//!
//! Run with `cargo run -p xtask -- <task>`, or through the `Justfile` recipes.
//!
//! This file is the dispatcher and nothing else. Each task is one module exposing one
//! `GATE`: the name it is invoked by, the sentence its holding justifies, the document to
//! read when it does not hold, and the check itself. The table below is the whole routing
//! decision, so a task is added by writing a module and one line here.
//!
//! Every gate here is written before the code it governs exists (docs/adr/), so the
//! first violation is caught on the commit that would introduce it. A gate that cannot
//! run reports failure, never success: an unreadable manifest says nothing about the
//! invariant.

mod bin_name;
mod deps;
mod purity;
mod quirkdb;
mod shared;
mod unsafe_boundary;

use std::process::ExitCode;

use crate::shared::Gate;

/// Every task, in the order ARCHITECTURE.md's gate table lists them.
const GATES: &[Gate] = &[
    purity::GATE,
    deps::GATE,
    unsafe_boundary::GATE,
    quirkdb::GATE,
    bin_name::GATE,
];

fn main() -> ExitCode {
    let Some(task) = std::env::args().nth(1) else {
        print_usage();
        return ExitCode::FAILURE;
    };
    let Some(gate) = GATES.iter().find(|gate| gate.name == task) else {
        eprintln!("xtask: unknown task `{task}`");
        print_usage();
        return ExitCode::FAILURE;
    };
    gate.report()
}

/// Print the available tasks and what each one states.
fn print_usage() {
    eprintln!("usage: cargo run -p xtask -- <task>");
    for gate in GATES {
        eprintln!(
            "  {name:<15}  {purpose}",
            name = gate.name,
            purpose = gate.purpose
        );
    }
}
