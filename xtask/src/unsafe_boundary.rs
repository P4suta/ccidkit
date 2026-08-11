// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `unsafe-boundary` — repository Rust source contains no unsafe code.

use std::fs;
use std::io;

use crate::shared::{Gate, code_of, holds_token, relative_name, rust_sources, workspace_root};

pub(crate) const GATE: Gate = Gate {
    name: "unsafe-boundary",
    purpose: "no workspace source contains an unsafe block or unsafe function",
    reference: "docs/adr/0016",
    run: check,
};

fn check() -> io::Result<Vec<String>> {
    let root = workspace_root()?;
    let mut violations = Vec::new();
    for source in rust_sources(&root.join("crates"))?
        .into_iter()
        .chain(rust_sources(&root.join("xtask").join("src"))?)
    {
        let name = relative_name(&source, &root);
        let text = fs::read_to_string(source)?;
        violations.extend(violations_in(&name, &text));
    }
    Ok(violations)
}

fn violations_in(name: &str, text: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let code = code_of(line)?;
            holds_token(&code, "unsafe").then(|| {
                format!(
                    "{name}:{line}: unsafe code is forbidden by ADR-0016",
                    line = index.saturating_add(1),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{check, violations_in};

    #[test]
    fn committed_workspace_has_no_unsafe_code() {
        assert_eq!(check().expect("the gate runs"), Vec::<String>::new());
    }

    #[test]
    fn code_is_rejected_but_prose_and_lint_names_are_allowed() {
        assert_eq!(violations_in("x.rs", "unsafe { call() }\n").len(), 1);
        assert_eq!(violations_in("x.rs", "unsafe fn call() {}\n").len(), 1);
        assert!(violations_in("x.rs", "// unsafe is discussed\n").is_empty());
        assert!(violations_in("x.rs", "#![forbid(unsafe_code)]\n").is_empty());
    }
}
