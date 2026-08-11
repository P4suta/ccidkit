// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `bin-name` — no binary shares a name with any library crate.
//!
//! Parallel rustdoc writes every target to `target/doc/<name>`, so a bin target named
//! like a lib crate makes `cargo doc` fail or silently overwrite (docs/adr/0015). The
//! frozen names keep the two sets disjoint — the facade is `ccidkit`, the binaries are
//! `ccid` and `ccdev` — and this gate keeps them disjoint as targets are added.
//!
//! A crate with a `src/main.rs` and no `[[bin]]` rename has an implicit bin target named
//! after the package, so that case is checked too rather than assumed away.

use std::fs;
use std::io;

use crate::shared::{Gate, array_header, before_comment, members, quoted_values, table_header};

/// The gate, as the dispatcher's table wants it.
pub(crate) const GATE: Gate = Gate {
    name: "bin-name",
    purpose: "every bin target's name differs from every lib crate's name",
    reference: "docs/adr/0015",
    run: check,
};

/// Collect lib names and bin names across the workspace and reject collisions.
fn check() -> io::Result<Vec<String>> {
    let all = members()?;
    let mut violations = Vec::new();

    let mut lib_names: Vec<String> = Vec::new();
    let mut bin_names: Vec<(String, String)> = Vec::new();

    for member in &all {
        if member.directory.join("src").join("lib.rs").is_file() {
            lib_names.push(member.name.clone());
        }
        let manifest = fs::read_to_string(member.directory.join("Cargo.toml"))?;
        let declared = declared_bin_names(&manifest);
        if declared.is_empty() {
            if member.directory.join("src").join("main.rs").is_file() {
                bin_names.push((member.name.clone(), member.name.clone()));
            }
        } else {
            for name in declared {
                bin_names.push((member.name.clone(), name));
            }
        }
    }

    for (package, bin) in &bin_names {
        if lib_names.contains(bin) {
            violations.push(format!(
                "{package}: bin target `{bin}` shares its name with a lib crate; \
                 parallel rustdoc fights over target/doc/{bin} (docs/adr/0015)"
            ));
        }
    }

    Ok(violations)
}

/// The `name` values of every `[[bin]]` table in a manifest.
fn declared_bin_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside_bin = false;
    for line in manifest.lines() {
        let line = before_comment(line).trim();
        if let Some(header) = array_header(line) {
            inside_bin = header == "bin";
            continue;
        }
        if table_header(line).is_some() {
            inside_bin = false;
            continue;
        }
        if inside_bin {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    if let Some(&name) = quoted_values(value).first() {
                        names.push(name.to_owned());
                    }
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::{check, declared_bin_names};

    #[test]
    fn the_bootstrap_workspace_keeps_the_sets_disjoint() {
        assert_eq!(check().expect("the gate runs"), Vec::<String>::new());
    }

    #[test]
    fn reads_every_bin_table() {
        let manifest = "[package]\nname = \"x\"\n\n[[bin]]\nname = \"one\"\n\n[[bin]]\nname = \"two\"\npath = \"src/two.rs\"\n";
        assert_eq!(declared_bin_names(manifest), ["one", "two"]);
    }

    #[test]
    fn a_lib_only_manifest_declares_no_bin() {
        assert!(declared_bin_names("[package]\nname = \"x\"\n").is_empty());
    }
}
