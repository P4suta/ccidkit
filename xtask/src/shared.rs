// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What every task shares: where the repository is, how to read it, and how a finding is
//! reported.
//!
//! A gate is four things — the name it is invoked by, the sentence its holding justifies,
//! the document to read when it does not hold, and the check itself. Reporting is written
//! once here so that every task speaks with the same voice and so that adding a task is
//! writing a module and one line in the dispatcher's table.
//!
//! The list of workspace members is *derived* rather than kept: `Cargo.toml` already
//! names them, and each gate subtracts the members its rule deliberately does not cover.
//! A crate added to the workspace and forgotten in a denylist is therefore checked and
//! fails the gate, where a hand-maintained allowlist would have skipped it in silence.
//!
//! Manifests are read by a deliberate hand-rolled scan rather than a TOML parser or
//! `cargo metadata`. The tool that enforces "the pure layer has no dependencies" declares
//! none itself, so it reads the one manifest style this repository writes and nothing
//! else — the same choice the kumihan and wimkit xtasks made before it.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// One repository gate: a subcommand of `xtask`.
#[derive(Debug)]
pub(crate) struct Gate {
    /// The name the gate is invoked by, after `cargo run -p xtask --`.
    pub(crate) name: &'static str,
    /// What holding this gate means, in one line. Printed when the check finds nothing,
    /// and listed in the usage message, so it states what was actually checked.
    pub(crate) purpose: &'static str,
    /// Where to read about the invariant. Printed after the violations when it fails.
    pub(crate) reference: &'static str,
    /// The check itself. It returns one message per violation, or an error when it
    /// could not run at all.
    pub(crate) run: fn() -> io::Result<Vec<String>>,
}

impl Gate {
    /// Run the gate and turn its findings into output and an exit code.
    ///
    /// A gate that cannot run is a failure, not a pass: an unreadable manifest tells us
    /// nothing about the invariant, and reporting success there would be the one failure
    /// mode a policy check must not have.
    pub(crate) fn report(&self) -> ExitCode {
        let Self {
            name,
            purpose,
            reference,
            run,
        } = self;
        match run() {
            Err(error) => {
                eprintln!("xtask: {name} could not run: {error}");
                ExitCode::FAILURE
            },
            Ok(violations) if violations.is_empty() => {
                println!("{name}: {purpose}");
                ExitCode::SUCCESS
            },
            Ok(violations) => {
                for violation in &violations {
                    eprintln!("{name}: {violation}");
                }
                eprintln!(
                    "{name}: {count} violation(s). See {reference}",
                    count = violations.len()
                );
                ExitCode::FAILURE
            },
        }
    }
}

/// One workspace member.
#[derive(Debug)]
pub(crate) struct Member {
    /// The package name, as its own manifest declares it.
    pub(crate) name: String,
    /// The directory holding that manifest.
    pub(crate) directory: PathBuf,
}

/// Locate the workspace root relative to this crate.
pub(crate) fn workspace_root() -> io::Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "the xtask manifest directory has no parent",
            )
        })
}

/// Every workspace member, in workspace order, named by the package name its own
/// manifest declares rather than by its directory, because a dependency is written with
/// the package name and the two are free to differ.
pub(crate) fn members() -> io::Result<Vec<Member>> {
    let root = workspace_root()?;
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let paths = workspace_members(&manifest);
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cargo.toml declares no workspace members",
        ));
    }

    let mut found = Vec::new();
    for path in paths {
        let directory = root.join(&path);
        let manifest = fs::read_to_string(directory.join("Cargo.toml"))?;
        let name = package_name(&manifest).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{path}/Cargo.toml declares no package name"),
            )
        })?;
        found.push(Member {
            name: name.to_owned(),
            directory,
        });
    }
    Ok(found)
}

/// Read the member paths out of a workspace manifest.
///
/// It understands the one form this repository writes — a `members` array under
/// `[workspace]`, on one line or several — and reads nothing else.
fn workspace_members(manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut inside_workspace = false;
    let mut inside_members = false;

    for line in manifest.lines() {
        let line = before_comment(line).trim();
        if line.is_empty() {
            continue;
        }

        if inside_members {
            found.extend(quoted_values(line).into_iter().map(str::to_owned));
            inside_members = !line.contains(']');
            continue;
        }

        if let Some(header) = table_header(line) {
            inside_workspace = header == "workspace";
            continue;
        }

        if inside_workspace {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "members" {
                    found.extend(quoted_values(value).into_iter().map(str::to_owned));
                    inside_members = !value.contains(']');
                }
            }
        }
    }

    found
}

/// Read the package name out of a crate manifest.
pub(crate) fn package_name(manifest: &str) -> Option<&str> {
    let mut inside_package = false;
    for line in manifest.lines() {
        let line = before_comment(line).trim();
        if let Some(header) = table_header(line) {
            inside_package = header == "package";
            continue;
        }
        if inside_package {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    return quoted_values(value).first().copied();
                }
            }
        }
    }
    None
}

/// The name inside a `[table]` header, if the line is one.
pub(crate) fn table_header(line: &str) -> Option<&str> {
    if line.starts_with("[[") {
        return None;
    }
    line.strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .map(str::trim)
}

/// The name inside an `[[array]]` header, if the line is one.
pub(crate) fn array_header(line: &str) -> Option<&str> {
    line.strip_prefix("[[")
        .and_then(|rest| rest.strip_suffix("]]"))
        .map(str::trim)
}

/// Everything before the first `#` that is not inside a string.
pub(crate) fn before_comment(line: &str) -> &str {
    let mut inside = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        match character {
            '\\' if inside => escaped = !escaped,
            '"' if !escaped => inside = !inside,
            '#' if !inside => return line.get(..index).unwrap_or(line),
            _ => escaped = false,
        }
    }
    line
}

/// The string literals on a line, in order, without their quotes.
pub(crate) fn quoted_values(line: &str) -> Vec<&str> {
    let mut values = Vec::new();
    let mut rest = line;
    while let Some((_, after)) = rest.split_once('"') {
        let Some((value, remainder)) = after.split_once('"') else {
            break;
        };
        values.push(value);
        rest = remainder;
    }
    values
}

/// One dependency a manifest declares.
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Declared {
    /// Whether it was declared in a `dev-dependencies` table.
    pub(crate) dev: bool,
    /// The package name, as the manifest spells it.
    pub(crate) name: String,
}

/// Which kind of dependency table a manifest header names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TableKind {
    /// `dependencies` or `build-dependencies`, in any `cfg`-gated spelling. Build
    /// dependencies count as normal because they reach the shipped artifact's build.
    Normal,
    /// `dev-dependencies`, in any `cfg`-gated spelling.
    Dev,
}

/// Every dependency a manifest declares, with the kind of table it came from.
///
/// It understands `[dependencies]` tables, `[dependencies.name]` sub-tables, and
/// `[target.'cfg(..)'.dependencies]`, and it skips comment lines, so a commented-out
/// dependency is not a violation.
pub(crate) fn declared_dependencies(manifest: &str) -> BTreeSet<Declared> {
    let mut names = BTreeSet::new();
    let mut inside = None;

    for line in manifest.lines() {
        let line = before_comment(line).trim();
        if line.is_empty() {
            continue;
        }

        let header = table_header(line).or_else(|| array_header(line));
        if let Some(header) = header {
            if let Some((kind, name)) = dependency_subtable(header) {
                names.insert(Declared {
                    dev: kind == TableKind::Dev,
                    name: name.to_owned(),
                });
                inside = None;
            } else {
                inside = dependency_table_kind(header);
            }
            continue;
        }

        if let Some(kind) = inside {
            if let Some((key, _)) = line.split_once('=') {
                let key = key.trim().trim_matches('"');
                if !key.is_empty() {
                    names.insert(Declared {
                        dev: kind == TableKind::Dev,
                        name: key.to_owned(),
                    });
                }
            }
        }
    }

    names
}

/// The kind of dependency table `header` names, if it names one.
fn dependency_table_kind(header: &str) -> Option<TableKind> {
    [
        ("dev-dependencies", TableKind::Dev),
        ("build-dependencies", TableKind::Normal),
        ("dependencies", TableKind::Normal),
    ]
    .into_iter()
    .find(|(suffix, _)| header == *suffix || header.ends_with(&format!(".{suffix}")))
    .map(|(_, kind)| kind)
}

/// Extract `name` from a `[dependencies.name]` style header.
fn dependency_subtable(header: &str) -> Option<(TableKind, &str)> {
    let (prefix, name) = header.rsplit_once('.')?;
    let kind = dependency_table_kind(prefix)?;
    Some((kind, name.trim_matches('"')))
}

/// Gather every `.rs` file under `dir`, recursively, in a stable order.
pub(crate) fn rust_sources(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    collect_rust_sources(dir, &mut found)?;
    found.sort();
    Ok(found)
}

/// Walk one directory, appending in whatever order the filesystem reports.
fn collect_rust_sources(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_sources(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// `path` relative to `root`, with forward slashes so a message reads the same everywhere.
pub(crate) fn relative_name(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// One line with its comment and its string literals removed, or `None` for pure prose.
///
/// A line-oriented approximation, adequate because it reads only this repository's own
/// sources: a line whose first non-space characters are `//` is prose, and a span
/// between two `"` is data. The worst case is ignoring a token inside a multi-line
/// string literal, which the sources this scans do not use for code-shaped text.
pub(crate) fn code_of(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') {
        return None;
    }
    let mut code = String::new();
    let mut inside_string = false;
    let mut previous = ' ';
    for character in trimmed.chars() {
        if character == '"' && previous != '\\' {
            inside_string = !inside_string;
            previous = character;
            continue;
        }
        if !inside_string {
            code.push(character);
        }
        previous = character;
    }
    Some(code)
}

/// Whether `line` contains `token` as a whole word rather than inside a longer
/// identifier.
pub(crate) fn holds_token(line: &str, token: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find(token) {
        let head = rest.get(..at).and_then(|before| before.chars().next_back());
        let tail = rest
            .get(at.saturating_add(token.len())..)
            .and_then(|after| after.chars().next());
        let bounded = |edge: Option<char>| {
            edge.is_none_or(|character| !character.is_alphanumeric() && character != '_')
        };
        if bounded(head) && bounded(tail) {
            return true;
        }
        let Some(next) = rest.get(at.saturating_add(1)..) else {
            break;
        };
        rest = next;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        Declared, code_of, declared_dependencies, holds_token, members, package_name,
        quoted_values, table_header, workspace_members,
    };

    #[test]
    fn reads_members_from_a_multi_line_array() {
        let manifest = "[workspace]\nresolver = \"3\"\nmembers = [\n  \"crates/a\",\n  \"xtask\",\n]\nexclude = [\"fuzz\"]\n";
        assert_eq!(workspace_members(manifest), ["crates/a", "xtask"]);
    }

    #[test]
    fn reads_members_only_from_the_workspace_table() {
        let manifest = "[workspace.metadata]\nmembers = [\"decoy\"]\n\n[workspace]\nmembers = [\"crates/a\"]\n";
        assert_eq!(workspace_members(manifest), ["crates/a"]);
    }

    #[test]
    fn a_commented_out_member_is_not_a_member() {
        let manifest = "[workspace]\nmembers = [\n  \"crates/a\",\n  # \"crates/b\",\n]\n";
        assert_eq!(workspace_members(manifest), ["crates/a"]);
    }

    #[test]
    fn reads_the_package_name_and_not_another_tables_name() {
        let manifest =
            "[package]\nname = \"ccid-apdu\"\nedition = \"2024\"\n\n[lints]\nname = \"decoy\"\n";
        assert_eq!(package_name(manifest), Some("ccid-apdu"));
        assert_eq!(package_name("[lints]\nname = \"decoy\"\n"), None);
    }

    #[test]
    fn a_table_header_is_a_whole_line() {
        assert_eq!(table_header("[package]"), Some("package"));
        assert_eq!(table_header("[[bin]]"), None);
        assert_eq!(table_header("members = [\"crates/a\"]"), None);
        assert_eq!(quoted_values("name = \"a\" # \"b\""), ["a", "b"]);
    }

    #[test]
    fn reads_dependencies_with_their_table_kind() {
        let manifest = "[dependencies]\nccid-apdu = { path = \"../ccid-apdu\" }\n\n[dev-dependencies]\nccid-testkit = { path = \"../ccid-testkit\" }\n";
        let names = declared_dependencies(manifest);
        assert!(names.contains(&Declared {
            dev: false,
            name: "ccid-apdu".to_owned()
        }));
        assert!(names.contains(&Declared {
            dev: true,
            name: "ccid-testkit".to_owned()
        }));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn reads_dependencies_from_subtables_and_target_tables() {
        let names = declared_dependencies(
            "[dependencies.nusb]\nversion = \"0.2\"\n\n[target.'cfg(windows)'.dependencies]\nwindows-sys = \"0.61\"\n",
        );
        assert!(names.contains(&Declared {
            dev: false,
            name: "nusb".to_owned()
        }));
        assert!(names.contains(&Declared {
            dev: false,
            name: "windows-sys".to_owned()
        }));
        assert_eq!(names.len(), 2, "subtable keys are not dependencies");
    }

    #[test]
    fn ignores_commented_and_empty_manifests() {
        assert!(declared_dependencies("").is_empty());
        assert!(declared_dependencies("[dependencies]\n").is_empty());
        assert!(declared_dependencies("[dependencies]\n# nusb = \"0.2\"\n").is_empty());
    }

    #[test]
    fn code_of_strips_comments_and_strings() {
        assert_eq!(code_of("// unsafe in prose"), None);
        assert_eq!(
            code_of("let x = \"unsafe\"; call();").as_deref(),
            Some("let x = ; call();")
        );
    }

    #[test]
    fn holds_token_matches_whole_words_only() {
        assert!(holds_token("unsafe {", "unsafe"));
        assert!(!holds_token("unsafely_named()", "unsafe"));
        assert!(!holds_token("an_unsafe_name", "unsafe"));
    }

    #[test]
    fn every_member_is_derived_with_a_manifest() {
        let found = members().expect("the workspace manifest is readable");
        let names: Vec<&str> = found.iter().map(|each| each.name.as_str()).collect();
        assert!(names.contains(&"ccid-apdu"), "found {names:?}");
        assert!(names.contains(&"ccidkit"), "found {names:?}");
        assert!(names.contains(&"xtask"), "found {names:?}");
        for each in &found {
            assert!(
                each.directory.join("Cargo.toml").is_file(),
                "{name} has a manifest",
                name = each.name
            );
        }
    }
}
