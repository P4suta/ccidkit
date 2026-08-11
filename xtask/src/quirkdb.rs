// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `quirkdb` — the reader quirk table keeps its schema and its receipts.
//!
//! docs/adr/0009 makes reader knowledge data with provenance: one `[[reader]]` entry per
//! misbehaving model, each carrying the reproduction that earned it. This gate makes the
//! schema mechanical: sorted, unique, fully attributed, and spoken in a closed flag
//! vocabulary — so review argues about whether a quirk is real, never about whether the
//! file is well-formed. It runs green over zero entries, which is how the schema is
//! enforced from the first entry rather than retrofitted.

use std::fs;
use std::io;

use crate::shared::{Gate, array_header, before_comment, quoted_values, table_header};

/// The table, relative to the repository root.
const QUIRK_TABLE: &str = "quirks/readers.toml";

/// The flag vocabulary. An entry may only use these; a new kind of misbehavior adds its
/// flag here (with the sentence saying what it means) in the same change that first uses
/// it, so the vocabulary and the table cannot drift apart.
const FLAGS: &[&str] = &[
    // The class descriptor misstates what the reader can do; trust the table entry.
    "bogus-descriptor",
    // Claims extended APDU but fails exchanges past the short-APDU boundary.
    "no-extended-apdu",
    // Bulk-out transfers need a zero-length packet after full packets.
    "needs-zlp",
    // The ATR is not valid until well after power-on reports success.
    "slow-power-on",
    // The advertised PIN pad is unusable; route PIN entry to the host.
    "pinpad-broken",
];

/// Where a reproduction may come from.
const SOURCES: &[&str] = &["cassette", "issue", "capture"];

/// The gate, as the dispatcher's table wants it.
pub(crate) const GATE: Gate = Gate {
    name: "quirkdb",
    purpose: "quirks/readers.toml is sorted, unique, attributed, and in vocabulary",
    reference: "docs/adr/0009",
    run: check,
};

/// One `[[reader]]` entry as the file spells it, fields optional until validated.
#[derive(Debug, Default)]
struct Entry {
    /// The line its header sits on, for messages.
    line: usize,
    /// USB vendor id.
    vid: Option<u32>,
    /// USB product id.
    pid: Option<u32>,
    /// Marketing name.
    name: Option<String>,
    /// Quirk flags.
    flags: Option<Vec<String>>,
    /// Provenance: where the reproduction lives.
    source: Option<String>,
    /// Provenance: the file or URL that reproduces it.
    evidence: Option<String>,
    /// Provenance: when the reproduction was made.
    date: Option<String>,
}

/// Which part of an entry the cursor is inside.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    /// Between the header comment and the first entry, where only comments belong.
    Preamble,
    /// Inside a `[[reader]]` entry's own keys.
    Reader,
    /// Inside a `[reader.provenance]` sub-table.
    Provenance,
}

/// Read and validate the table.
fn check() -> io::Result<Vec<String>> {
    let path = crate::shared::workspace_root()?.join(QUIRK_TABLE);
    let text = fs::read_to_string(path)?;
    Ok(violations_in(&text))
}

/// Every violation in the table's text.
fn violations_in(text: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut section = Section::Preamble;

    for (index, raw) in text.lines().enumerate() {
        let at = index.saturating_add(1);
        let line = before_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(header) = array_header(line) {
            if header == "reader" {
                entries.push(Entry {
                    line: at,
                    ..Entry::default()
                });
                section = Section::Reader;
            } else {
                violations.push(format!(
                    "{QUIRK_TABLE}:{at}: unknown array `[[{header}]]`; the table holds \
                     only `[[reader]]` entries"
                ));
            }
            continue;
        }
        if let Some(header) = table_header(line) {
            if header == "reader.provenance" && section != Section::Preamble {
                section = Section::Provenance;
            } else {
                violations.push(format!(
                    "{QUIRK_TABLE}:{at}: unknown table `[{header}]`; an entry holds only \
                     `[reader.provenance]`"
                ));
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            violations.push(format!("{QUIRK_TABLE}:{at}: not a `key = value` line"));
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let Some(entry) = entries.last_mut() else {
            violations.push(format!(
                "{QUIRK_TABLE}:{at}: `{key}` outside any `[[reader]]` entry"
            ));
            continue;
        };

        match (section, key) {
            (Section::Reader, "vid") => entry.vid = read_id(value, at, "vid", &mut violations),
            (Section::Reader, "pid") => entry.pid = read_id(value, at, "pid", &mut violations),
            (Section::Reader, "name") => {
                entry.name = quoted_values(value).first().map(|&name| name.to_owned());
            },
            (Section::Reader, "flags") => {
                entry.flags = Some(
                    quoted_values(value)
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                );
            },
            (Section::Provenance, "source") => {
                entry.source = quoted_values(value).first().map(|&name| name.to_owned());
            },
            (Section::Provenance, "evidence") => {
                entry.evidence = quoted_values(value).first().map(|&name| name.to_owned());
            },
            (Section::Provenance, "date") => {
                entry.date = quoted_values(value).first().map(|&name| name.to_owned());
            },
            _ => violations.push(format!(
                "{QUIRK_TABLE}:{at}: unknown key `{key}` for this section; the schema is \
                 in the file's header comment"
            )),
        }
    }

    validate(&entries, &mut violations);
    violations
}

/// Parse a `vid` or `pid` value: hex with `0x`, or decimal, at most `0xFFFF`.
fn read_id(value: &str, at: usize, key: &str, violations: &mut Vec<String>) -> Option<u32> {
    let parsed = value.strip_prefix("0x").map_or_else(
        || value.parse().ok(),
        |hex| u32::from_str_radix(hex, 16).ok(),
    );
    match parsed {
        Some(id) if id <= 0xFFFF => Some(id),
        Some(_) => {
            violations.push(format!(
                "{QUIRK_TABLE}:{at}: `{key}` does not fit in sixteen bits"
            ));
            None
        },
        None => {
            violations.push(format!(
                "{QUIRK_TABLE}:{at}: `{key}` is not a `0x`-hex or decimal integer"
            ));
            None
        },
    }
}

/// The cross-entry rules: completeness, vocabulary, order, uniqueness.
fn validate(entries: &[Entry], violations: &mut Vec<String>) {
    let mut previous: Option<(u32, u32)> = None;
    for entry in entries {
        let at = entry.line;
        for (field, present) in [
            ("vid", entry.vid.is_some()),
            ("pid", entry.pid.is_some()),
            ("name", entry.name.is_some()),
            ("flags", entry.flags.is_some()),
            ("provenance.source", entry.source.is_some()),
            ("provenance.evidence", entry.evidence.is_some()),
            ("provenance.date", entry.date.is_some()),
        ] {
            if !present {
                violations.push(format!(
                    "{QUIRK_TABLE}:{at}: entry is missing `{field}`; every quirk carries \
                     its identity and its receipt (docs/adr/0009)"
                ));
            }
        }

        for flag in entry.flags.iter().flatten() {
            if !FLAGS.contains(&flag.as_str()) {
                violations.push(format!(
                    "{QUIRK_TABLE}:{at}: flag `{flag}` is not in the vocabulary in \
                     xtask/src/quirkdb.rs"
                ));
            }
        }
        if let Some(source) = &entry.source {
            if !SOURCES.contains(&source.as_str()) {
                violations.push(format!(
                    "{QUIRK_TABLE}:{at}: provenance source `{source}` is not one of \
                     {SOURCES:?}"
                ));
            }
        }
        if let Some(date) = &entry.date {
            if !date_shaped(date) {
                violations.push(format!(
                    "{QUIRK_TABLE}:{at}: provenance date `{date}` is not `YYYY-MM-DD`"
                ));
            }
        }

        if let (Some(vid), Some(pid)) = (entry.vid, entry.pid) {
            if let Some(before) = previous {
                if before == (vid, pid) {
                    violations.push(format!(
                        "{QUIRK_TABLE}:{at}: duplicate entry for {vid:04x}:{pid:04x}"
                    ));
                } else if before > (vid, pid) {
                    violations.push(format!(
                        "{QUIRK_TABLE}:{at}: entries are not sorted ascending by \
                         (vid, pid)"
                    ));
                }
            }
            previous = Some((vid, pid));
        }
    }
}

/// Whether `date` is shaped `YYYY-MM-DD`. A shape check, not a calendar.
fn date_shaped(date: &str) -> bool {
    let bytes = date.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::{check, date_shaped, violations_in};

    /// A complete, well-formed single entry.
    const GOOD: &str = "[[reader]]\nvid = 0x08e6\npid = 0x3437\nname = \"Example Reader\"\nflags = [\"bogus-descriptor\"]\n\n[reader.provenance]\nsource = \"cassette\"\nevidence = \"cassettes/example-3437.toml\"\ndate = \"2026-08-11\"\n";

    #[test]
    fn the_committed_table_is_valid() {
        assert_eq!(check().expect("the gate runs"), Vec::<String>::new());
    }

    #[test]
    fn a_comment_only_table_is_valid_and_empty() {
        assert!(violations_in("# only prose\n\n# more prose\n").is_empty());
    }

    #[test]
    fn a_complete_entry_is_valid() {
        assert_eq!(violations_in(GOOD), Vec::<String>::new());
    }

    #[test]
    fn a_missing_provenance_is_a_violation() {
        let text = "[[reader]]\nvid = 0x08e6\npid = 0x3437\nname = \"X\"\nflags = []\n";
        let found = violations_in(text);
        assert!(
            found
                .iter()
                .any(|violation| violation.contains("provenance.source")),
            "found {found:?}"
        );
    }

    #[test]
    fn an_unknown_flag_is_a_violation() {
        let text = GOOD.replace("bogus-descriptor", "made-up-flag");
        let found = violations_in(&text);
        assert!(
            found
                .iter()
                .any(|violation| violation.contains("made-up-flag")),
            "found {found:?}"
        );
    }

    #[test]
    fn order_and_duplicates_are_violations() {
        let two = format!("{GOOD}\n{GOOD}");
        let found = violations_in(&two);
        assert!(
            found
                .iter()
                .any(|violation| violation.contains("duplicate")),
            "found {found:?}"
        );

        let second = GOOD.replace("0x08e6", "0x0001");
        let unsorted = format!("{GOOD}\n{second}");
        let found = violations_in(&unsorted);
        assert!(
            found.iter().any(|violation| violation.contains("sorted")),
            "found {found:?}"
        );
    }

    #[test]
    fn ids_must_fit_sixteen_bits() {
        let text = GOOD.replace("0x08e6", "0x10000");
        let found = violations_in(&text);
        assert!(
            found
                .iter()
                .any(|violation| violation.contains("sixteen bits")),
            "found {found:?}"
        );
    }

    #[test]
    fn dates_are_shape_checked() {
        assert!(date_shaped("2026-08-11"));
        assert!(!date_shaped("2026-8-11"));
        assert!(!date_shaped("11 Aug 2026"));
    }
}
