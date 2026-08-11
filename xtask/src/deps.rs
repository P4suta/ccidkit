// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `deps` — every member's edges against the one dependency matrix.
//!
//! ARCHITECTURE.md draws the deliberately tiny package graph; this checks it. The rules, keyed to what each
//! defends:
//!
//! * **R1 — the one rule.** A member's normal dependency on another member must be an
//!   arrow [`ALLOWED`] carries.
//! * **R2 — the tooling charter.** A [`ZERO_DEP`] crate declares no normal dependency.
//! * **R3 — no dev cycle.** A dev-dependency ships in nothing, so R1 does not reach it;
//!   it may not close a cycle, which would state the architecture backwards.
//! * **R5 — the testkit is dev-only.** A [`DEV_ONLY`] crate may be named only from a
//!   dev-dependency table, so it reaches no shipped artifact.
//! * **R7 — total coverage.** Every workspace member has an [`ALLOWED`] row and every
//!   row names a member, so a new crate cannot slip past by being unlisted and the
//!   matrix cannot rot.
//!
//! The numbering is inherited from the deps gate this is modeled on; source purity and
//! the no-unsafe rule are separate gates because Cargo edges cannot express them.

use std::fs;
use std::io;

use crate::shared::{Gate, declared_dependencies, members};

/// Which workspace crates each crate may reach. **Transitively closed**, so a single
/// lookup answers "may `from` name `to`, directly or through anything it names" (see
/// [`reaches`]), and the closure is what the tests below hold.
///
/// A row is reachability, not intent: `ccid-cli`'s row carries everything the facade
/// reaches because the closure demands it, while the intended *direct* edge — the CLI
/// names `ccidkit` and nothing else — is stated in ARCHITECTURE.md and in each crate
/// manifest's own comment. The rows are ordered as `Cargo.toml` orders the members.
const ALLOWED: &[(&str, &[&str])] = &[
    ("ccidkit", &[]),
    ("ccid-cli", &["ccidkit"]),
    ("ccid-driverkit", &["ccidkit"]),
    ("xtask", &[]),
];

/// Repository tooling stays dependency-free so architecture gates remain easy to audit.
const ZERO_DEP: &[&str] = &["xtask"];

/// Crates that may be named only from a dev-dependency table.
const DEV_ONLY: &[&str] = &[];

/// The gate, as the dispatcher's table wants it.
pub(crate) const GATE: Gate = Gate {
    name: "deps",
    purpose: "every dependency edge is an arrow the ALLOWED matrix carries",
    reference: "docs/adr/0016 and ARCHITECTURE.md",
    run: check,
};

/// Whether `from` may reach `to`, directly or through anything it may name.
///
/// One lookup, because [`ALLOWED`] is transitively closed.
fn reaches(from: &str, to: &str) -> bool {
    ALLOWED
        .iter()
        .find(|(name, _)| *name == from)
        .is_some_and(|(_, allowed)| allowed.contains(&to))
}

/// Check every workspace member against the rules above.
fn check() -> io::Result<Vec<String>> {
    let all = members()?;
    let is_member = |name: &str| all.iter().any(|member| member.name == name);
    let mut violations = Vec::new();

    // R7, both directions.
    for member in &all {
        if !ALLOWED.iter().any(|(name, _)| *name == member.name) {
            violations.push(format!(
                "{name}: workspace member has no row in xtask/src/deps.rs ALLOWED — add \
                 one so the one rule reaches it",
                name = member.name,
            ));
        }
    }
    for (krate, _) in ALLOWED {
        if !is_member(krate) {
            violations.push(format!(
                "{krate}: has an ALLOWED row but is not a workspace member; the matrix \
                 in xtask/src/deps.rs has gone stale"
            ));
        }
    }

    for member in &all {
        let Some((_, allowed)) = ALLOWED.iter().find(|(name, _)| *name == member.name) else {
            continue;
        };
        let manifest = fs::read_to_string(member.directory.join("Cargo.toml"))?;
        let on_charter = ZERO_DEP.contains(&member.name.as_str());

        for declared in declared_dependencies(&manifest) {
            let dep = declared.name.as_str();

            // R2 — a charter crate declares no normal dependency of any kind.
            if on_charter && !declared.dev {
                violations.push(format!(
                    "{name}: declares `{dep}`; this crate's dependency-freedom is \
                     architecture (docs/adr/0016)",
                    name = member.name,
                ));
                continue;
            }

            if !is_member(dep) {
                continue;
            }

            // R5 — a dev-only crate is a dev-dependency and nothing else.
            if DEV_ONLY.contains(&dep) && !declared.dev {
                violations.push(format!(
                    "{name}: names `{dep}` as a normal dependency; it is dev-only and \
                     must reach no shipped artifact",
                    name = member.name,
                ));
                continue;
            }

            if declared.dev {
                // R3 — a dev-dependency may not close a cycle: a crate whose tests are
                // written in the terms of something above it has inverted, whatever the
                // shipped artifact holds.
                if reaches(dep, &member.name) {
                    violations.push(format!(
                        "{name}: dev-dependency `{dep}` closes a cycle — this crate is \
                         below the one it is testing with",
                        name = member.name,
                    ));
                }
            } else if !allowed.contains(&dep) {
                // R1 — the one rule.
                violations.push(format!(
                    "{name}: normal dependency `{dep}` is not an arrow the ALLOWED \
                     matrix carries (ARCHITECTURE.md)",
                    name = member.name,
                ));
            }
        }
    }

    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED, DEV_ONLY, ZERO_DEP, check, reaches};

    #[test]
    fn the_bootstrap_workspace_obeys_the_matrix() {
        assert_eq!(check().expect("the gate runs"), Vec::<String>::new());
    }

    #[test]
    fn the_matrix_is_transitively_closed() {
        // The claim `reaches` rests on: if A may name B, then A may name everything B
        // may. A single lookup is only sound over a closed matrix.
        for (krate, allowed) in ALLOWED {
            for dep in *allowed {
                let (_, deps_of_dep) = ALLOWED
                    .iter()
                    .find(|(name, _)| name == dep)
                    .unwrap_or_else(|| panic!("{krate} may name {dep}, which has no row"));
                for transitive in *deps_of_dep {
                    assert!(
                        allowed.contains(transitive),
                        "{krate} may name {dep}, which may name {transitive} — but \
                         {krate} may not. ALLOWED must be transitively closed."
                    );
                }
            }
        }
    }

    #[test]
    fn no_crate_may_name_itself() {
        for (krate, allowed) in ALLOWED {
            assert!(!allowed.contains(krate), "{krate} names itself");
        }
    }

    #[test]
    fn every_charter_crate_has_an_empty_row() {
        // The two rules must agree: a crate that may depend on nothing has nothing in
        // its row.
        for krate in ZERO_DEP {
            let (_, allowed) = ALLOWED
                .iter()
                .find(|(name, _)| name == krate)
                .unwrap_or_else(|| panic!("{krate} is on the charter but has no ALLOWED row"));
            assert!(
                allowed.is_empty(),
                "{krate} is on the charter but may name {allowed:?}"
            );
        }
    }

    #[test]
    fn nothing_reaches_a_dev_only_crate() {
        // R5 stated against the matrix itself: no row carries an arrow to the testkit,
        // so a normal edge to it is unrepresentable as well as rejected.
        for krate in DEV_ONLY {
            for (from, allowed) in ALLOWED {
                assert!(
                    !allowed.contains(krate),
                    "{from} may reach {krate}, which is dev-only"
                );
            }
        }
    }

    #[test]
    fn the_intended_direct_edges_read_as_the_design_says() {
        // Spot checks that the closure carries the design's sentences.
        assert!(reaches("ccid-cli", "ccidkit"), "the CLI sits on the facade");
        assert!(
            reaches("ccid-driverkit", "ccidkit"),
            "the driver tool proves the diagnostics facade"
        );
        assert!(
            !reaches("ccidkit", "ccid-cli"),
            "nothing depends on a binary"
        );
    }
}
