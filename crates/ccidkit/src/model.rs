// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as StdError;
use std::fmt;

use crate::{Error, ErrorKind, Result};

const MAX_COMMAND_DATA: usize = u16::MAX as usize;
const MAX_EXPECTED_LENGTH: usize = 65_536;

/// A validated ISO/IEC 7816 command APDU.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Command {
    class: u8,
    instruction: u8,
    parameter1: u8,
    parameter2: u8,
    data: Box<[u8]>,
    expected_length: Option<u32>,
    extended: bool,
}

impl Command {
    /// Construct a case-1 command from its four-byte header.
    #[must_use]
    pub fn new(class: u8, instruction: u8, parameter1: u8, parameter2: u8) -> Self {
        Self {
            class,
            instruction,
            parameter1,
            parameter2,
            data: Box::new([]),
            expected_length: None,
            extended: false,
        }
    }

    /// Parse one canonical short or extended command APDU.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let header: [u8; 4] = bytes
            .get(..4)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidInput,
                    "a command APDU needs a four-byte header",
                )
            })?
            .try_into()
            .map_err(|_| malformed_command())?;
        let mut command = Self::new(header[0], header[1], header[2], header[3]);
        let Some(length_marker) = bytes.get(4).copied() else {
            return Ok(command);
        };

        if bytes.len() == 5 {
            command.expected_length = Some(if length_marker == 0 {
                256
            } else {
                u32::from(length_marker)
            });
            return Ok(command);
        }

        if length_marker != 0 {
            let data_length = usize::from(length_marker);
            let data_end = 5_usize.checked_add(data_length).ok_or_else(length_error)?;
            let expected_end = data_end.checked_add(1).ok_or_else(length_error)?;
            if bytes.len() != data_end && bytes.len() != expected_end {
                return Err(malformed_command());
            }
            command.data = bytes
                .get(5..data_end)
                .ok_or_else(malformed_command)?
                .to_vec()
                .into_boxed_slice();
            if bytes.len() == expected_end {
                let encoded = bytes.get(data_end).copied().ok_or_else(malformed_command)?;
                command.expected_length = Some(if encoded == 0 {
                    256
                } else {
                    u32::from(encoded)
                });
            }
            return Ok(command);
        }

        let length_bytes: [u8; 2] = bytes
            .get(5..7)
            .ok_or_else(malformed_command)?
            .try_into()
            .map_err(|_| malformed_command())?;
        command.extended = true;
        let encoded_length = u16::from_be_bytes([length_bytes[0], length_bytes[1]]);
        if bytes.len() == 7 {
            command.expected_length = Some(if encoded_length == 0 {
                65_536
            } else {
                u32::from(encoded_length)
            });
            return Ok(command);
        }
        if encoded_length == 0 {
            return Err(malformed_command());
        }

        let data_length = usize::from(encoded_length);
        let data_end = 7_usize.checked_add(data_length).ok_or_else(length_error)?;
        let expected_end = data_end.checked_add(2).ok_or_else(length_error)?;
        if bytes.len() != data_end && bytes.len() != expected_end {
            return Err(malformed_command());
        }
        command.data = bytes
            .get(7..data_end)
            .ok_or_else(malformed_command)?
            .to_vec()
            .into_boxed_slice();
        if bytes.len() == expected_end {
            let encoded: [u8; 2] = bytes
                .get(data_end..expected_end)
                .ok_or_else(malformed_command)?
                .try_into()
                .map_err(|_| malformed_command())?;
            let value = u16::from_be_bytes([encoded[0], encoded[1]]);
            command.expected_length = Some(if value == 0 { 65_536 } else { u32::from(value) });
        }
        Ok(command)
    }

    /// Replace the command data, choosing short or extended encoding automatically.
    pub fn with_data(mut self, data: impl Into<Vec<u8>>) -> Result<Self> {
        let data = data.into();
        if data.len() > MAX_COMMAND_DATA {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "command data exceeds the 65,535-byte APDU limit",
            ));
        }
        self.data = data.into_boxed_slice();
        self.extended |= self.data.len() > usize::from(u8::MAX);
        Ok(self)
    }

    /// Set the maximum response length from 1 through 65,536 bytes.
    pub fn with_expected_len(mut self, expected_length: usize) -> Result<Self> {
        if !(1..=MAX_EXPECTED_LENGTH).contains(&expected_length) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "expected APDU response length must be in 1..=65,536",
            ));
        }
        self.expected_length = u32::try_from(expected_length).ok();
        self.extended |= expected_length > 256;
        Ok(self)
    }

    /// Encode the command in its shortest canonical representation.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut encoded = vec![
            self.class,
            self.instruction,
            self.parameter1,
            self.parameter2,
        ];
        let use_extended = self.extended;

        if self.data.is_empty() {
            if let Some(expected) = self.expected_length {
                if use_extended {
                    encoded.push(0);
                    encoded.extend_from_slice(&encode_extended_length(expected));
                } else {
                    encoded.push(encode_short_length(expected));
                }
            }
            return encoded;
        }

        if use_extended {
            encoded.push(0);
            let length = u16::try_from(self.data.len()).map_or(0, |value| value);
            encoded.extend_from_slice(&length.to_be_bytes());
            encoded.extend_from_slice(&self.data);
            if let Some(expected) = self.expected_length {
                encoded.extend_from_slice(&encode_extended_length(expected));
            }
        } else {
            let length = u8::try_from(self.data.len()).map_or(0, |value| value);
            encoded.push(length);
            encoded.extend_from_slice(&self.data);
            if let Some(expected) = self.expected_length {
                encoded.push(encode_short_length(expected));
            }
        }
        encoded
    }

    /// Return the class byte.
    #[must_use]
    pub const fn class(&self) -> u8 {
        self.class
    }

    /// Return the instruction byte.
    #[must_use]
    pub const fn instruction(&self) -> u8 {
        self.instruction
    }

    /// Return the first parameter byte.
    #[must_use]
    pub const fn parameter1(&self) -> u8 {
        self.parameter1
    }

    /// Return the second parameter byte.
    #[must_use]
    pub const fn parameter2(&self) -> u8 {
        self.parameter2
    }

    /// Return command data without its length encoding.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Return the requested response length, if any.
    #[must_use]
    pub const fn expected_len(&self) -> Option<u32> {
        self.expected_length
    }

    pub(crate) fn with_replaced_expected_len(&self, expected: usize) -> Result<Self> {
        self.clone().with_expected_len(expected)
    }

    #[cfg(any(
        target_os = "linux",
        all(target_os = "windows", feature = "native-usb")
    ))]
    pub(crate) const fn uses_extended_encoding(&self) -> bool {
        self.extended
    }
}

fn encode_short_length(length: u32) -> u8 {
    if length == 256 {
        0
    } else {
        u8::try_from(length).map_or(0, |value| value)
    }
}

fn encode_extended_length(length: u32) -> [u8; 2] {
    if length == 65_536 {
        [0, 0]
    } else {
        u16::try_from(length).map_or([0, 0], u16::to_be_bytes)
    }
}

fn malformed_command() -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        "malformed or non-canonical command APDU",
    )
}

fn length_error() -> Error {
    Error::new(ErrorKind::InvalidInput, "APDU length arithmetic overflow")
}

/// The two-byte status word returned by a card.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatusWord(u16);

impl StatusWord {
    /// Construct a status word from SW1 and SW2.
    #[must_use]
    pub const fn new(sw1: u8, sw2: u8) -> Self {
        Self(u16::from_be_bytes([sw1, sw2]))
    }

    /// Construct a status word from its big-endian integer representation.
    #[must_use]
    pub const fn from_u16(value: u16) -> Self {
        Self(value)
    }

    /// Return SW1 and SW2.
    #[must_use]
    pub const fn bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Return the big-endian integer representation.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Whether the card reported ordinary success (`9000`).
    #[must_use]
    pub const fn is_success(self) -> bool {
        self.0 == 0x9000
    }

    /// A concise interpretation for common interindustry status words.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self.0 {
            0x9000 => "success",
            0x6282 => "end of file reached before reading expected bytes",
            0x6300 => "authentication failed or state unchanged",
            0x6700 => "wrong length",
            0x6982 => "security status not satisfied",
            0x6983 => "authentication method blocked",
            0x6985 => "conditions of use not satisfied",
            0x6A80 => "incorrect data",
            0x6A81 => "function not supported",
            0x6A82 => "file or application not found",
            0x6A86 => "incorrect parameters",
            0x6D00 => "instruction not supported",
            0x6E00 => "class not supported",
            value if value & 0xFF00 == 0x6100 => "more response bytes available",
            value if value & 0xFF00 == 0x6C00 => "wrong response length; SW2 is correct Le",
            _ => "card-defined status",
        }
    }
}

impl fmt::Display for StatusWord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:04X}", self.0)
    }
}

/// A response APDU: zero or more data bytes followed by a status word.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Response {
    data: Box<[u8]>,
    status: StatusWord,
}

impl Response {
    /// Construct a response from data and an explicit status word.
    #[must_use]
    pub fn new(data: impl Into<Vec<u8>>, status: StatusWord) -> Self {
        Self {
            data: data.into().into_boxed_slice(),
            status,
        }
    }

    /// Parse response bytes, requiring the final two status bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let split = bytes.len().checked_sub(2).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidInput,
                "a response APDU needs two status bytes",
            )
        })?;
        let (data, status) = bytes.split_at(split);
        let status: [u8; 2] = status
            .get(..2)
            .ok_or_else(|| {
                Error::new(ErrorKind::InvalidInput, "response status word is truncated")
            })?
            .try_into()
            .map_err(|_| {
                Error::new(ErrorKind::InvalidInput, "response status word is truncated")
            })?;
        Ok(Self::new(
            data.to_vec(),
            StatusWord::new(status[0], status[1]),
        ))
    }

    /// Encode data and status bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.data.to_vec();
        bytes.extend_from_slice(&self.status.bytes());
        bytes
    }

    /// Return response data without SW1/SW2.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Consume the response and return its data.
    #[must_use]
    pub fn into_data(self) -> Box<[u8]> {
        self.data
    }

    /// Return the card's status word.
    #[must_use]
    pub const fn status(&self) -> StatusWord {
        self.status
    }

    /// Keep successful responses and turn other card statuses into explicit policy.
    pub fn require_success(self) -> std::result::Result<Self, StatusError> {
        if self.status.is_success() {
            Ok(self)
        } else {
            Err(StatusError { response: self })
        }
    }
}

/// A caller-requested failure for a response whose status is not `9000`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusError {
    response: Response,
}

impl StatusError {
    /// Return the complete response, including any warning data.
    #[must_use]
    pub const fn response(&self) -> &Response {
        &self.response
    }

    /// Recover ownership of the complete response.
    #[must_use]
    pub fn into_response(self) -> Response {
        self.response
    }
}

impl fmt::Display for StatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "card returned {} ({})",
            self.response.status,
            self.response.status.meaning()
        )
    }
}

impl StdError for StatusError {}

/// A validated answer-to-reset with parsed protocol and historical-byte boundaries.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Atr {
    bytes: Box<[u8]>,
    historical_start: usize,
    historical_end: usize,
    protocols: Box<[u8]>,
}

impl Atr {
    /// Parse and validate an ISO/IEC 7816 answer-to-reset.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let convention = bytes.first().copied().ok_or_else(invalid_atr)?;
        if !matches!(convention, 0x3B | 0x3F) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "ATR convention byte must be 3B or 3F",
            ));
        }
        let format = bytes.get(1).copied().ok_or_else(invalid_atr)?;
        let historical_length = usize::from(format & 0x0F);
        let mut presence = format >> 4;
        let mut offset = 2_usize;
        let mut protocols = Vec::new();

        loop {
            for mask in [0x1_u8, 0x2, 0x4] {
                if presence & mask != 0 {
                    take_atr_byte(bytes, &mut offset)?;
                }
            }
            if presence & 0x8 == 0 {
                break;
            }
            let descriptor = take_atr_byte(bytes, &mut offset)?;
            let protocol = descriptor & 0x0F;
            if !protocols.contains(&protocol) {
                protocols.push(protocol);
            }
            presence = descriptor >> 4;
        }

        if protocols.is_empty() {
            protocols.push(0);
        }

        let historical_start = offset;
        let historical_end = historical_start
            .checked_add(historical_length)
            .ok_or_else(invalid_atr)?;
        if bytes.get(historical_start..historical_end).is_none() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "ATR historical bytes are truncated",
            ));
        }
        let needs_checksum = protocols.iter().any(|protocol| *protocol != 0);
        let expected_length = historical_end
            .checked_add(usize::from(needs_checksum))
            .ok_or_else(invalid_atr)?;
        if bytes.len() != expected_length {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "ATR contains trailing bytes or omits its checksum",
            ));
        }
        if needs_checksum {
            let checksum = bytes
                .get(1..expected_length)
                .ok_or_else(invalid_atr)?
                .iter()
                .fold(0_u8, |value, byte| value ^ byte);
            if checksum != 0 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "ATR checksum does not reduce to zero",
                ));
            }
        }
        Ok(Self {
            bytes: bytes.to_vec().into_boxed_slice(),
            historical_start,
            historical_end,
            protocols: protocols.into_boxed_slice(),
        })
    }

    /// Return the original, validated ATR bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the historical bytes declared by T0.
    #[must_use]
    pub fn historical_bytes(&self) -> &[u8] {
        self.bytes
            .get(self.historical_start..self.historical_end)
            .unwrap_or_default()
    }

    /// Iterate the advertised protocol numbers. T=0 is included when implicit.
    #[must_use]
    pub fn protocols(&self) -> impl ExactSizeIterator<Item = u8> + '_ {
        self.protocols.iter().copied()
    }
}

fn invalid_atr() -> Error {
    Error::new(ErrorKind::InvalidInput, "ATR is truncated")
}

fn take_atr_byte(bytes: &[u8], offset: &mut usize) -> Result<u8> {
    let value = bytes.get(*offset).copied().ok_or_else(invalid_atr)?;
    *offset = offset.checked_add(1).ok_or_else(invalid_atr)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{Atr, Command, Response, StatusWord};

    #[test]
    fn command_round_trips_all_four_short_cases() {
        let fixtures: &[&[u8]] = &[
            &[0x00, 0x84, 0x00, 0x00],
            &[0x00, 0x84, 0x00, 0x00, 0x08],
            &[0x00, 0xA4, 0x04, 0x00, 0x02, 0x3F, 0x00],
            &[0x00, 0xA4, 0x04, 0x00, 0x02, 0x3F, 0x00, 0x00],
        ];
        for fixture in fixtures {
            let parsed = Command::from_bytes(fixture);
            assert!(matches!(parsed, Ok(ref command) if command.to_bytes() == *fixture));
        }
    }

    #[test]
    fn command_round_trips_extended_cases() {
        let cases: &[&[u8]] = &[
            &[0x00, 0xCA, 0x00, 0x00, 0x00, 0x01, 0x00],
            &[0x00, 0xDA, 0x00, 0x00, 0x00, 0x00, 0x02, 0xAA, 0xBB],
            &[
                0x00, 0xDA, 0x00, 0x00, 0x00, 0x00, 0x02, 0xAA, 0xBB, 0x01, 0x00,
            ],
        ];
        for fixture in cases {
            let parsed = Command::from_bytes(fixture);
            assert!(matches!(parsed, Ok(ref command) if command.to_bytes() == *fixture));
        }
    }

    #[test]
    fn command_rejects_every_ambiguous_or_truncated_length_shape() {
        let malformed: &[&[u8]] = &[
            &[],
            &[0, 1, 2],
            &[0, 0xA4, 0, 0, 2, 0xAA],
            &[0, 0xA4, 0, 0, 1, 0xAA, 0, 0],
            &[0, 0xA4, 0, 0, 0, 1],
            &[0, 0xA4, 0, 0, 0, 0, 0, 0xAA],
            &[0, 0xA4, 0, 0, 0, 0, 2, 0xAA],
        ];
        for bytes in malformed {
            assert!(Command::from_bytes(bytes).is_err(), "accepted {bytes:02X?}");
        }
    }

    #[test]
    fn command_builders_hold_all_length_boundaries() {
        let short = Command::new(0, 0xCA, 0, 0)
            .with_expected_len(256)
            .expect("short Le");
        assert_eq!(short.to_bytes(), [0, 0xCA, 0, 0, 0]);

        let extended = Command::new(0, 0xCA, 0, 0)
            .with_expected_len(257)
            .expect("extended Le");
        assert_eq!(extended.to_bytes(), [0, 0xCA, 0, 0, 0, 1, 1]);
        assert!(Command::new(0, 0, 0, 0).with_expected_len(0).is_err());
        assert!(Command::new(0, 0, 0, 0).with_expected_len(65_537).is_err());
        assert!(Command::new(0, 0, 0, 0).with_data(vec![0; 65_536]).is_err());
    }

    #[test]
    fn explicit_extended_wire_form_is_preserved() {
        let bytes = [0, 0xCA, 0, 0, 0, 1, 0];
        let command = Command::from_bytes(&bytes).expect("extended Le=256");
        assert_eq!(command.expected_len(), Some(256));
        assert_eq!(command.to_bytes(), bytes);

        let short_case4 = Command::from_bytes(&[0, 0xDA, 0, 0, 1, 0xAA, 0]).expect("short case 4");
        assert_eq!(short_case4.expected_len(), Some(256));
    }

    #[test]
    fn status_is_data_until_the_caller_requires_success() {
        let response = Response::new(Vec::new(), StatusWord::from_u16(0x6A82));
        assert_eq!(response.status().meaning(), "file or application not found");
        assert!(response.require_success().is_err());
    }

    #[test]
    fn atr_parses_historical_bytes_and_checksum() {
        let simple = Atr::parse(&[0x3B, 0x02, 0x11, 0x22]);
        assert!(matches!(simple, Ok(ref atr) if atr.historical_bytes() == [0x11, 0x22]));

        let with_t1 = Atr::parse(&[0x3B, 0x80, 0x01, 0x81]);
        assert!(matches!(with_t1, Ok(ref atr) if atr.protocols().eq([1])));
    }

    #[test]
    fn atr_rejects_bad_convention_truncation_checksum_and_trailing_data() {
        for malformed in [
            Vec::new(),
            vec![0x00, 0x00],
            vec![0x3B, 0x10],
            vec![0x3B, 0x01],
            vec![0x3B, 0x00, 0xAA],
            vec![0x3B, 0x80, 0x01, 0x80],
        ] {
            assert!(Atr::parse(&malformed).is_err(), "accepted {malformed:02X?}");
        }
    }

    #[test]
    fn atr_follows_more_than_one_interface_descriptor_group() {
        let atr = Atr::parse(&[0x3B, 0x80, 0x81, 0x01, 0x00]).expect("two TD groups");
        assert!(atr.protocols().eq([1]));
        assert_eq!(atr.as_bytes(), [0x3B, 0x80, 0x81, 0x01, 0x00]);
    }
}
