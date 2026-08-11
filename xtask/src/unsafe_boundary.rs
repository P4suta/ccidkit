// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `unsafe-boundary` — `unsafe` lives in one crate, and behaves there.
//!
//! The workspace deliberately does not `forbid(unsafe_code)` (docs/adr/0011): the PC/SC
//! shim fronts a C API and may need `unsafe` at the boundary. A Clippy lint cannot
//! express "only this directory", so this task does, with two rules:
//!
//! 1. No source outside [`UNSAFE_DIRECTORY`] contains the token `unsafe`.
//! 2. Every `unsafe {` inside it is preceded by a comment block holding a `// SAFETY:`
//!    line. Redundant with `clippy::undocumented_unsafe_blocks`, and kept because it is
//!    the check that survives a Clippy version bump moving a restriction lint.
//!
//! What `unsafe fn` bodies must look like is owned by the workspace lint
//! `unsafe_op_in_unsafe_fn = "deny"`: an unsafe operation inside one still needs its own
//! block, and that block lands under rule 2.

use std::fs;
use std::io;

use crate::shared::{Gate, code_of, holds_token, relative_name, rust_sources, workspace_root};

/// The one directory in the workspace where `unsafe` may appear.
///
/// Relative to the repository root, spelled with forward slashes. The policy this
/// enforces is written out in `crates/ccid-backend-pcsc/src/lib.rs` and docs/adr/0011.
const UNSAFE_DIRECTORY: &str = "crates/ccid-backend-pcsc/src";

/// The comment every `unsafe` block must sit under.
const SAFETY: &str = "// SAFETY:";

/// The gate, as the dispatcher's table wants it.
pub(crate) const GATE: Gate = Gate {
    name: "unsafe-boundary",
    purpose: "`unsafe` appears only in the quarantine, each block under a SAFETY comment",
    reference: "docs/adr/0011",
    run: check,
};

/// Check every source in the workspace against the two rules.
fn check() -> io::Result<Vec<String>> {
    let root = workspace_root()?;
    let quarantine = root.join(UNSAFE_DIRECTORY);
    let mut violations = Vec::new();

    for source in rust_sources(&root.join("crates"))?
        .into_iter()
        .chain(rust_sources(&root.join("xtask").join("src"))?)
    {
        let name = relative_name(&source, &root);
        let text = fs::read_to_string(&source)?;
        if source.starts_with(&quarantine) {
            violations.extend(check_inside(&name, &text));
        } else {
            violations.extend(check_outside(&name, &text));
        }
    }
    Ok(violations)
}

/// Rule 1, applied to a file that may not say `unsafe` at all.
///
/// Comments and string literals are excluded, so prose — including this gate's own
/// messages — may name the token it looks for.
fn check_outside(name: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(code) = code_of(line) else {
            continue;
        };
        if holds_token(&code, "unsafe") {
            violations.push(format!(
                "{name}:{at}: `unsafe` outside {UNSAFE_DIRECTORY}/ (docs/adr/0011)",
                at = index.saturating_add(1),
            ));
        }
    }
    violations
}

/// Rule 2, applied to the quarantine.
fn check_inside(name: &str, text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(code) = code_of(line) else {
            continue;
        };
        if !code.contains("unsafe {") {
            continue;
        }
        if !documented_above(&lines, index) {
            violations.push(format!(
                "{name}:{at}: an `unsafe {{` whose comment block above it holds no \
                 `{SAFETY}` line",
                at = index.saturating_add(1),
            ));
        }
    }
    violations
}

/// Whether the contiguous comment block immediately above `at` carries a `// SAFETY:`
/// line.
///
/// The block rather than the single line above, because a comment that establishes
/// handle validity, buffer provenance, initialization, and the failure contract does not
/// fit on one line and should not be written as though it did.
fn documented_above(lines: &[&str], at: usize) -> bool {
    let mut above = at;
    while let Some(index) = above.checked_sub(1) {
        let Some(line) = lines.get(index) else {
            return false;
        };
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            return false;
        }
        if trimmed.starts_with(SAFETY) {
            return true;
        }
        above = index;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{check, check_inside, check_outside};

    #[test]
    fn the_bootstrap_workspace_holds_the_boundary() {
        assert_eq!(check().expect("the gate runs"), Vec::<String>::new());
    }

    #[test]
    fn prose_may_name_the_token() {
        assert!(check_outside("a.rs", "// unsafe is discussed here\n").is_empty());
        assert!(check_outside("a.rs", "let s = \"unsafe\";\n").is_empty());
    }

    #[test]
    fn code_outside_may_not() {
        let found = check_outside("a.rs", "fn f() { unsafe { g() } }\n");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn a_block_needs_its_safety_comment() {
        let documented =
            "// SAFETY: the handle outlives the call\n// and the buffer is ours.\nunsafe { g() }\n";
        assert!(check_inside("a.rs", documented).is_empty());
        let undocumented = "// a comment that is not the required one\nunsafe { g() }\n";
        assert_eq!(check_inside("a.rs", undocumented).len(), 1);
        assert_eq!(check_inside("a.rs", "unsafe { g() }\n").len(), 1);
    }
}
