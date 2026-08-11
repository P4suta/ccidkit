// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `purity` — the pure layer takes nothing from outside itself, dev-dependencies
//! included.
//!
//! The pure layer is every workspace member this module does not exempt. That direction
//! is deliberate: a crate added to the workspace is checked as pure until someone writes
//! down why it is not, so the failure mode is a loud gate rather than a silent hole. The
//! layer's members — `ccid-apdu`, `ccid-core`, `ccid-proto`, `ccid-backend-virtual` —
//! may declare dependencies only on each other, in any table. This is what makes
//! docs/adr/0013's "no third party arrives transitively" true by construction: a layer
//! closed under itself cannot import anything through a member.

use std::fs;
use std::io;

use crate::shared::{Gate, declared_dependencies, members};

/// Workspace members that are deliberately not part of the pure layer.
///
/// A denylist rather than an allowlist, because the two fail in opposite directions and
/// only one of them fails safely. Each entry is a member path exactly as `Cargo.toml`
/// spells it, and an entry naming something that is no longer a member is an error
/// rather than a no-op, so this list cannot rot unnoticed.
const NON_PURE_MEMBERS: &[&str] = &[
    // Dev-only helpers; kept dependency-free by the deps gate's ZERO_DEP rule instead.
    "crates/ccid-testkit",
    // Owns real USB I/O and will take `nusb` (docs/adr/0001).
    "crates/ccid-backend-usb",
    // The shim: links the platform PC/SC service and is the unsafe quarantine
    // (docs/adr/0007, 0011).
    "crates/ccid-backend-pcsc",
    // The facade composes the backends, so it reaches whatever they reach.
    "crates/ccidkit",
    // The binaries sit above the facade and may take argument parsing and the like.
    "crates/ccid-cli",
    "crates/ccid-driverkit",
    // The repository's own tooling, which is this program.
    "xtask",
];

/// The gate, as the dispatcher's table wants it.
pub(crate) const GATE: Gate = Gate {
    name: "purity",
    purpose: "the pure layer declares no dependency outside itself, dev included",
    reference: "docs/adr/0013",
    run: check,
};

/// Check every pure crate's manifest against the closure rule.
fn check() -> io::Result<Vec<String>> {
    let all = members()?;
    let mut violations = Vec::new();

    for exempted in NON_PURE_MEMBERS {
        let expected = all
            .iter()
            .any(|member| member.directory.ends_with(exempted));
        if !expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "`{exempted}` is exempted from the pure layer but is not a workspace \
                     member; the exemption list in xtask/src/purity.rs has gone stale"
                ),
            ));
        }
    }

    let pure: Vec<_> = all
        .iter()
        .filter(|member| {
            !NON_PURE_MEMBERS
                .iter()
                .any(|exempted| member.directory.ends_with(exempted))
        })
        .collect();
    let pure_names: Vec<&str> = pure.iter().map(|member| member.name.as_str()).collect();

    for member in &pure {
        let manifest = fs::read_to_string(member.directory.join("Cargo.toml"))?;
        for declared in declared_dependencies(&manifest) {
            if !pure_names.contains(&declared.name.as_str()) {
                let table = if declared.dev { "dev-" } else { "" };
                violations.push(format!(
                    "{crate_name}: declares {table}dependency `{name}`; the pure layer \
                     may depend only on itself (docs/adr/0013)",
                    crate_name = member.name,
                    name = declared.name,
                ));
            }
        }
    }

    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::{NON_PURE_MEMBERS, check};

    #[test]
    fn the_bootstrap_workspace_is_pure() {
        assert_eq!(check().expect("the gate runs"), Vec::<String>::new());
    }

    #[test]
    fn the_denylist_is_paths_not_names() {
        for exempted in NON_PURE_MEMBERS {
            assert!(
                *exempted == "xtask" || exempted.starts_with("crates/"),
                "{exempted} is spelled as Cargo.toml spells members"
            );
        }
    }
}
