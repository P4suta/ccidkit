// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generate the private native-reader quirk lookup from the packaged TOML database.

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

mod build_support;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=quirks/readers.toml");
    let manifest = env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("CARGO_MANIFEST_DIR is unavailable"))?;
    let table = fs::read_to_string(manifest.join("quirks/readers.toml"))?;
    let generated = build_support::generate(&table);
    let output = env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("OUT_DIR is unavailable"))?;
    fs::write(output.join("ccidkit_quirks.rs"), generated)
}
