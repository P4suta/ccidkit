// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pure parser and renderer for the packaged native-reader quirk table.

use std::fmt::Write as _;

#[derive(Default)]
struct Entry {
    vid: Option<u16>,
    pid: Option<u16>,
    flags: Vec<String>,
}

pub(crate) fn generate(table: &str) -> String {
    render(&parse(table))
}

fn parse(table: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut current: Option<Entry> = None;
    for raw in table.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line == "[[reader]]" {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(Entry::default());
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "vid" => entry.vid = parse_id(value),
            "pid" => entry.pid = parse_id(value),
            "flags" => entry.flags = quoted(value),
            _ => {},
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    entries
}

fn parse_id(value: &str) -> Option<u16> {
    let value = value.trim();
    value.strip_prefix("0x").map_or_else(
        || value.parse().ok(),
        |hex| u16::from_str_radix(hex, 16).ok(),
    )
}

fn quoted(value: &str) -> Vec<String> {
    value
        .split('"')
        .enumerate()
        .filter(|(index, _)| index & 1 == 1)
        .map(|(_, part)| part.to_owned())
        .collect()
}

fn render(entries: &[Entry]) -> String {
    let mut output = String::from(
        "#[derive(Clone, Copy, Debug, Default)]\n\
         pub(crate) struct Quirks {\n\
         \x20   pub(crate) force_short_apdu: bool,\n\
         \x20   pub(crate) needs_zlp: bool,\n\
         \x20   pub(crate) slow_power_on: bool,\n\
         }\n\n\
         pub(crate) fn lookup(vid: u16, pid: u16) -> Quirks {\n",
    );
    let mut has_entries = false;
    for entry in entries {
        let (Some(vid), Some(pid)) = (entry.vid, entry.pid) else {
            continue;
        };
        if !has_entries {
            output.push_str("    match (vid, pid) {\n");
            has_entries = true;
        }
        let _ = writeln!(
            output,
            "        (0x{vid:04X}, 0x{pid:04X}) => Quirks {{ force_short_apdu: {}, needs_zlp: {}, slow_power_on: {} }},",
            entry.flags.iter().any(|flag| flag == "no-extended-apdu"),
            entry.flags.iter().any(|flag| flag == "needs-zlp"),
            entry.flags.iter().any(|flag| flag == "slow-power-on"),
        );
    }
    if has_entries {
        output.push_str("        _ => Quirks::default(),\n    }\n}\n");
    } else {
        output.push_str("    let _ = (vid, pid);\n    Quirks::default()\n}\n");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::generate;

    #[test]
    fn generator_maps_transport_flags_and_ignores_incomplete_entries() {
        let table = "[[reader]]\nvid = 0x1234\npid = 0xabcd\nflags = [\"needs-zlp\", \"slow-power-on\"]\n\n[[reader]]\nvid = 7\n";
        let generated = generate(table);
        assert!(generated.contains("(0x1234, 0xABCD)"));
        assert!(generated.contains("force_short_apdu: false"));
        assert!(generated.contains("needs_zlp: true"));
        assert!(generated.contains("slow_power_on: true"));
        assert!(!generated.contains("0x0007"));
    }

    #[test]
    fn empty_table_generates_a_total_default_lookup() {
        let generated = generate("# intentionally empty\n");
        assert!(generated.contains("let _ = (vid, pid)"));
        assert!(generated.contains("Quirks::default()"));
    }
}
