// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `purity` — parser and protocol modules remain sans-I/O inside the one library.

use std::fs;
use std::io;

use crate::shared::{Gate, code_of, workspace_root};

/// Pure files are kept inside the library instead of being semver-bearing crates.
const PURE_SOURCES: &[&str] = &[
    "crates/ccidkit/src/model.rs",
    "crates/ccidkit/src/ccid.rs",
    "crates/ccidkit/src/protocol.rs",
];

/// Imports that would give a pure state transformation an effect or backend identity.
const FORBIDDEN: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::sync",
    "std::thread",
    "std::time",
    "crate::backend",
    "crate::facade",
    "crate::operation",
    "crate::testing",
    "nusb",
    "pcsc",
];

pub(crate) const GATE: Gate = Gate {
    name: "purity",
    purpose: "the parser and protocol modules contain no I/O, clock, worker, or backend imports",
    reference: "docs/adr/0016",
    run: check,
};

fn check() -> io::Result<Vec<String>> {
    let root = workspace_root()?;
    let mut violations = Vec::new();
    for relative in PURE_SOURCES {
        let path = root.join(relative);
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("pure source `{relative}` is missing; update the gate with the move"),
            ));
        }
        let text = fs::read_to_string(path)?;
        violations.extend(violations_in(relative, &text));
    }
    Ok(violations)
}

fn violations_in(name: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(code) = code_of(line) else {
            continue;
        };
        for forbidden in FORBIDDEN {
            if code.contains(forbidden) {
                violations.push(format!(
                    "{name}:{line}: pure module names `{forbidden}` (docs/adr/0016)",
                    line = index.saturating_add(1),
                ));
            }
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::{check, violations_in};

    #[test]
    fn committed_protocol_sources_are_pure() {
        assert_eq!(check().expect("the gate runs"), Vec::<String>::new());
    }

    #[test]
    fn effects_are_rejected_but_prose_is_not() {
        assert_eq!(violations_in("x.rs", "use std::time::Instant;\n").len(), 1);
        assert!(violations_in("x.rs", "// std::time is discussed\n").is_empty());
    }
}
