// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Minimal command-line access to ccidkit's stable facade.

use std::env;
use std::process::ExitCode;

use ccidkit::{Command, Context, Error, Result};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ccid: {error}");
            ExitCode::FAILURE
        },
    }
}

fn run(arguments: &[String]) -> Result<()> {
    let Some(command) = arguments.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    match command {
        "list" => list(),
        "atr" => atr(),
        "apdu" => apdu(arguments.get(1..).unwrap_or_default()),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        },
        _ => Err(Error::new(
            ccidkit::ErrorKind::InvalidInput,
            format!("unknown command `{command}`; run `ccid help`"),
        )),
    }
}

fn list() -> Result<()> {
    let context = Context::new().wait()?;
    let readers = context.readers().wait()?;
    if readers.is_empty() {
        println!("No readers found.");
        return Ok(());
    }
    for reader in readers {
        let capabilities = reader.capabilities();
        println!(
            "{}  {:?}  {}\n  slots={} exchange={:?} max-message={} T=0:{} T=1:{}",
            reader.id(),
            reader.backend(),
            reader.name(),
            capabilities.slots(),
            capabilities.exchange_level(),
            capabilities.maximum_message_length(),
            yes_no(capabilities.supports_t0()),
            yes_no(capabilities.supports_t1()),
        );
    }
    Ok(())
}

fn atr() -> Result<()> {
    let card = ccidkit::open_first().wait()?;
    println!("{}", encode_hex(card.atr().as_bytes()));
    Ok(())
}

fn apdu(arguments: &[String]) -> Result<()> {
    let (raw, hex) = match arguments {
        [flag, rest @ ..] if flag == "--raw" => (true, rest),
        rest => (false, rest),
    };
    if hex.is_empty() {
        return Err(Error::new(
            ccidkit::ErrorKind::InvalidInput,
            "usage: ccid apdu [--raw] HEX",
        ));
    }
    let bytes = decode_hex(&hex.join(""))?;
    let command = Command::from_bytes(&bytes)?;
    let mut card = ccidkit::open_first().wait()?;
    let response = if raw {
        card.transmit_raw(command).wait()?
    } else {
        card.transmit(command).wait()?
    };
    println!("{}", encode_hex(&response.to_bytes()));
    Ok(())
}

fn decode_hex(input: &str) -> Result<Vec<u8>> {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && !matches!(character, ':' | '_'))
        .collect();
    if compact.len() % 2 != 0 {
        return Err(Error::new(
            ccidkit::ErrorKind::InvalidInput,
            "hex input must contain complete byte pairs",
        ));
    }
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|_| {
                Error::new(ccidkit::ErrorKind::InvalidInput, "hex input is not ASCII")
            })?;
            u8::from_str_radix(text, 16).map_err(|_| {
                Error::new(
                    ccidkit::ErrorKind::InvalidInput,
                    format!("`{text}` is not a hexadecimal byte"),
                )
            })
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02X}");
    }
    encoded
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn print_help() {
    println!(
        "ccid — small smart-card command line\n\n\
         Usage:\n  \
           ccid list\n  \
           ccid atr\n  \
           ccid apdu [--raw] HEX\n\n\
         `apdu` absorbs 6Cxx/61xx card flow by default; --raw performs one exchange."
    );
}

#[cfg(test)]
mod tests {
    use super::{decode_hex, encode_hex};

    #[test]
    fn hex_accepts_human_separators_and_round_trips() {
        let decoded = decode_hex("00:A4 04_00").expect("decode");
        assert_eq!(decoded, [0, 0xA4, 4, 0]);
        assert_eq!(encode_hex(&decoded), "00A40400");
    }

    #[test]
    fn hex_rejects_partial_and_non_hex_bytes() {
        assert!(decode_hex("0").is_err());
        assert!(decode_hex("GG").is_err());
    }
}
