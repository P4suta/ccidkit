// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Maintainer-facing reader diagnosis through ccidkit's stable diagnostics.

use std::env;
use std::process::ExitCode;

use ccidkit::{BackendKind, Context, Error, ErrorKind, Result};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ccdev: {error}");
            ExitCode::FAILURE
        },
    }
}

fn run(arguments: &[String]) -> Result<()> {
    match arguments.first().map(String::as_str) {
        Some("doctor") => doctor(parse_backend(arguments.get(1).map(String::as_str))?),
        Some("help" | "--help" | "-h") | None => {
            help();
            Ok(())
        },
        Some(other) => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unknown command `{other}`; run `ccdev help`"),
        )),
    }
}

fn parse_backend(value: Option<&str>) -> Result<Option<BackendKind>> {
    match value {
        None => Ok(None),
        Some("native" | "usb") => Ok(Some(BackendKind::NativeUsb)),
        Some("pcsc") => Ok(Some(BackendKind::Pcsc)),
        Some(other) => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unknown backend `{other}`; expected `native` or `pcsc`"),
        )),
    }
}

fn doctor(backend: Option<BackendKind>) -> Result<()> {
    let builder = backend.map_or_else(Context::builder, |backend| {
        Context::builder().backend(backend)
    });
    let context = builder.open().wait()?;
    println!("backend: {:?}", context.backend());
    let readers = context.readers().wait()?;
    println!("readers: {}", readers.len());
    for reader in readers {
        let capabilities = reader.capabilities();
        println!("\n{}  {}", reader.id(), reader.name());
        println!("  exchange: {:?}", capabilities.exchange_level());
        println!("  slots: {}", capabilities.slots());
        println!(
            "  maximum message: {} bytes",
            capabilities.maximum_message_length()
        );
        println!(
            "  protocols: {}{}",
            if capabilities.supports_t0() {
                "T=0 "
            } else {
                ""
            },
            if capabilities.supports_t1() {
                "T=1"
            } else {
                ""
            },
        );
        match reader.connect().wait() {
            Ok(card) => {
                let atr = card.atr();
                println!("  card: present");
                println!("  ATR: {}", hex(atr.as_bytes()));
                let protocols: Vec<String> = atr
                    .protocols()
                    .map(|protocol| format!("T={protocol}"))
                    .collect();
                println!("  ATR protocols: {}", protocols.join(", "));
                println!("  verdict: usable through the stable ccidkit surface");
            },
            Err(error) if error.kind() == ErrorKind::CardAbsent => {
                println!("  card: absent (reader path is healthy)");
            },
            Err(error) => {
                println!(
                    "  card: probe failed ({:?}: {})",
                    error.kind(),
                    error.message()
                );
                println!("  verdict: capture this output before proposing a quirk");
            },
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02X}");
    }
    encoded
}

fn help() {
    println!(
        "ccdev — ccidkit reader diagnostics\n\n\
         Usage:\n  ccdev doctor [native|pcsc]"
    );
}

#[cfg(test)]
mod tests {
    use super::parse_backend;
    use ccidkit::BackendKind;

    #[test]
    fn backend_names_are_deliberately_small() {
        assert!(matches!(
            parse_backend(Some("usb")),
            Ok(Some(BackendKind::NativeUsb))
        ));
        assert!(parse_backend(Some("invented")).is_err());
    }
}
