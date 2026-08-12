// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pure CCID descriptor and bulk-message codecs.

use crate::{Error, ErrorKind, Result};

pub(crate) const PC_TO_RDR_ICC_POWER_ON: u8 = 0x62;
pub(crate) const PC_TO_RDR_ICC_POWER_OFF: u8 = 0x63;
pub(crate) const PC_TO_RDR_GET_PARAMETERS: u8 = 0x6C;
pub(crate) const PC_TO_RDR_XFR_BLOCK: u8 = 0x6F;
pub(crate) const RDR_TO_PC_DATA_BLOCK: u8 = 0x80;
pub(crate) const RDR_TO_PC_SLOT_STATUS: u8 = 0x81;
pub(crate) const RDR_TO_PC_PARAMETERS: u8 = 0x82;

const HEADER_LENGTH: usize = 10;
const HEADER_LENGTH_U32: u32 = 10;
const CLASS_DESCRIPTOR_TYPE: u8 = 0x21;
const CLASS_DESCRIPTOR_LENGTH: usize = 54;
const CLASS_DESCRIPTOR_LENGTH_U8: u8 = 54;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorExchange {
    Character,
    Tpdu,
    ShortApdu,
    ExtendedApdu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DescriptorFacts {
    pub(crate) slots: u8,
    pub(crate) maximum_message_length: u32,
    pub(crate) exchange: DescriptorExchange,
    pub(crate) supports_t0: bool,
    pub(crate) supports_t1: bool,
}

pub(crate) fn parse_class_descriptor(bytes: &[u8]) -> Result<DescriptorFacts> {
    if bytes.len() != CLASS_DESCRIPTOR_LENGTH
        || bytes.first().copied() != Some(CLASS_DESCRIPTOR_LENGTH_U8)
        || bytes.get(1).copied() != Some(CLASS_DESCRIPTOR_TYPE)
    {
        return Err(protocol_error("malformed CCID class descriptor"));
    }
    let max_slot_index = byte(bytes, 4)?;
    let slots = max_slot_index.checked_add(1).ok_or_else(|| {
        protocol_error("CCID descriptor declares more slots than the API can address")
    })?;
    let protocols = little_u32(bytes, 6)?;
    let features = little_u32(bytes, 40)?;
    let maximum_message_length = little_u32(bytes, 44)?;
    if maximum_message_length < HEADER_LENGTH_U32 {
        return Err(protocol_error(
            "CCID maximum message length is smaller than its header",
        ));
    }
    let exchange = match features & 0x0007_0000 {
        0x0001_0000 => DescriptorExchange::Tpdu,
        0x0002_0000 => DescriptorExchange::ShortApdu,
        0x0004_0000 => DescriptorExchange::ExtendedApdu,
        _ => DescriptorExchange::Character,
    };
    Ok(DescriptorFacts {
        slots,
        maximum_message_length,
        exchange,
        supports_t0: protocols & 0x01 == 0x01,
        supports_t1: protocols & 0x02 == 0x02,
    })
}

pub(crate) fn power_on(slot: u8, sequence: u8) -> Vec<u8> {
    host_message(PC_TO_RDR_ICC_POWER_ON, slot, sequence, [0, 0, 0], &[]).unwrap_or_default()
}

pub(crate) fn power_off(slot: u8, sequence: u8) -> Vec<u8> {
    host_message(PC_TO_RDR_ICC_POWER_OFF, slot, sequence, [0, 0, 0], &[]).unwrap_or_default()
}

pub(crate) fn get_parameters(slot: u8, sequence: u8) -> Vec<u8> {
    host_message(PC_TO_RDR_GET_PARAMETERS, slot, sequence, [0, 0, 0], &[]).unwrap_or_default()
}

pub(crate) fn transfer_block(slot: u8, sequence: u8, payload: &[u8]) -> Result<Vec<u8>> {
    host_message(PC_TO_RDR_XFR_BLOCK, slot, sequence, [0, 0, 0], payload)
}

fn host_message(
    message_type: u8,
    slot: u8,
    sequence: u8,
    parameters: [u8; 3],
    payload: &[u8],
) -> Result<Vec<u8>> {
    let length = u32::try_from(payload.len()).map_err(|_| {
        Error::new(
            ErrorKind::InvalidInput,
            "CCID payload exceeds its 32-bit length field",
        )
    })?;
    let mut message = Vec::with_capacity(HEADER_LENGTH.saturating_add(payload.len()));
    message.push(message_type);
    message.extend_from_slice(&length.to_le_bytes());
    message.push(slot);
    message.push(sequence);
    message.extend_from_slice(&parameters);
    message.extend_from_slice(payload);
    Ok(message)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IccStatus {
    Active,
    Inactive,
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandStatus {
    Complete,
    Failed,
    TimeExtension,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeviceMessage {
    pub(crate) message_type: u8,
    pub(crate) slot: u8,
    pub(crate) sequence: u8,
    pub(crate) icc_status: IccStatus,
    pub(crate) command_status: CommandStatus,
    pub(crate) error: u8,
    pub(crate) specific: u8,
    pub(crate) payload: Box<[u8]>,
}

impl DeviceMessage {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        let header = bytes
            .get(..HEADER_LENGTH)
            .ok_or_else(|| protocol_error("CCID response header is truncated"))?;
        let payload_length = usize::try_from(little_u32(header, 1)?).map_err(|_| {
            protocol_error("CCID response length is not representable on this platform")
        })?;
        let expected = HEADER_LENGTH
            .checked_add(payload_length)
            .ok_or_else(|| protocol_error("CCID response length overflows"))?;
        if bytes.len() != expected {
            return Err(protocol_error(
                "CCID response length field does not match transfer",
            ));
        }
        let status = byte(header, 7)?;
        let icc_status = match status & 0x03 {
            0 => IccStatus::Active,
            1 => IccStatus::Inactive,
            2 => IccStatus::Absent,
            _ => return Err(protocol_error("CCID response has a reserved ICC status")),
        };
        let command_status = match status & 0xC0 {
            0x00 => CommandStatus::Complete,
            0x40 => CommandStatus::Failed,
            0x80 => CommandStatus::TimeExtension,
            _ => {
                return Err(protocol_error(
                    "CCID response has a reserved command status",
                ));
            },
        };
        Ok(Self {
            message_type: byte(header, 0)?,
            slot: byte(header, 5)?,
            sequence: byte(header, 6)?,
            icc_status,
            command_status,
            error: byte(header, 8)?,
            specific: byte(header, 9)?,
            payload: bytes
                .get(HEADER_LENGTH..)
                .unwrap_or_default()
                .to_vec()
                .into_boxed_slice(),
        })
    }

    pub(crate) fn validate_for(&self, expected_type: u8, slot: u8, sequence: u8) -> Result<()> {
        if self.message_type != expected_type {
            return Err(protocol_error(
                "reader returned the wrong CCID message type",
            ));
        }
        if self.slot != slot || self.sequence != sequence {
            return Err(protocol_error(
                "reader returned a stale CCID slot or sequence",
            ));
        }
        if self.icc_status == IccStatus::Absent {
            return Err(Error::from_kind(ErrorKind::CardGone));
        }
        if self.command_status == CommandStatus::Failed {
            return Err(Error::new(
                ErrorKind::Protocol,
                format!("CCID command failed with reader error 0x{:02X}", self.error),
            ));
        }
        Ok(())
    }

    pub(crate) fn transport_parameters(&self) -> Result<TransportParameters> {
        match (self.specific, self.payload.as_ref()) {
            (0, [_, _, _, _, _]) => Ok(TransportParameters::T0),
            (1, [_, checksum, _, _, _, ifsc, nad]) if *ifsc != 0 && *nad == 0 => {
                Ok(TransportParameters::T1 {
                    ifsc: *ifsc,
                    uses_crc: checksum & 0x01 != 0,
                })
            },
            (0 | 1, _) => Err(protocol_error(
                "reader returned malformed CCID protocol parameters",
            )),
            _ => Err(Error::new(
                ErrorKind::NotSupported,
                "reader selected a protocol other than T=0 or T=1",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransportParameters {
    T0,
    T1 { ifsc: u8, uses_crc: bool },
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| protocol_error("CCID field is truncated"))
}

fn little_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| protocol_error("CCID field offset overflows"))?;
    let field: [u8; 4] = bytes
        .get(offset..end)
        .ok_or_else(|| protocol_error("CCID 32-bit field is truncated"))?
        .try_into()
        .map_err(|_| protocol_error("CCID 32-bit field has the wrong length"))?;
    Ok(u32::from_le_bytes(field))
}

fn protocol_error(message: &'static str) -> Error {
    Error::new(ErrorKind::Protocol, message)
}

#[cfg(test)]
mod tests {
    use super::{
        CommandStatus, DescriptorExchange, DeviceMessage, IccStatus, RDR_TO_PC_DATA_BLOCK,
        RDR_TO_PC_PARAMETERS, RDR_TO_PC_SLOT_STATUS, TransportParameters, get_parameters,
        parse_class_descriptor, power_off, power_on, transfer_block,
    };

    fn parameter_message(protocol: u8, payload: &[u8]) -> DeviceMessage {
        let length = u32::try_from(payload.len()).expect("test payload length");
        let mut bytes = Vec::with_capacity(10_usize.saturating_add(payload.len()));
        bytes.push(RDR_TO_PC_PARAMETERS);
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&[0, 1, 0, 0, protocol]);
        bytes.extend_from_slice(payload);
        DeviceMessage::decode(&bytes).expect("CCID parameters envelope")
    }

    #[test]
    fn transfer_block_has_canonical_little_endian_header() {
        let encoded = transfer_block(2, 7, &[0x00, 0x84, 0x00, 0x00]).expect("encode");
        assert_eq!(encoded, [0x6F, 4, 0, 0, 0, 2, 7, 0, 0, 0, 0, 0x84, 0, 0]);
        assert_eq!(power_on(2, 7), [0x62, 0, 0, 0, 0, 2, 7, 0, 0, 0]);
        assert_eq!(power_off(2, 7), [0x63, 0, 0, 0, 0, 2, 7, 0, 0, 0]);
        assert_eq!(get_parameters(2, 7), [0x6C, 0, 0, 0, 0, 2, 7, 0, 0, 0]);
    }

    #[test]
    fn protocol_parameters_preserve_t0_and_t1_negotiation() {
        let t0 = parameter_message(0, &[0x11, 0, 0, 0, 0]);
        assert_eq!(
            t0.transport_parameters().expect("parse T=0"),
            TransportParameters::T0
        );

        let t1 = |checksum| parameter_message(1, &[0x11, checksum, 0, 0x4D, 0, 0x40, 0]);
        assert_eq!(
            t1(0x11).transport_parameters().expect("parse T=1 CRC"),
            TransportParameters::T1 {
                ifsc: 0x40,
                uses_crc: true,
            }
        );
        assert_eq!(
            t1(0x10).transport_parameters().expect("parse T=1 LRC"),
            TransportParameters::T1 {
                ifsc: 0x40,
                uses_crc: false,
            }
        );
    }

    #[test]
    fn protocol_parameters_distinguish_malformed_and_unknown_protocols() {
        let malformed: [(u8, &[u8]); 3] = [
            (0, &[]),
            (1, &[0x11, 0, 0, 0, 0, 0, 0]),
            (1, &[0x11, 0, 0, 0, 0, 32, 1]),
        ];
        for (protocol, payload) in malformed {
            assert_eq!(
                parameter_message(protocol, payload)
                    .transport_parameters()
                    .expect_err("known protocol must reject malformed parameters")
                    .kind(),
                crate::ErrorKind::Protocol
            );
        }
        assert_eq!(
            parameter_message(2, &[])
                .transport_parameters()
                .expect_err("unknown protocol")
                .kind(),
            crate::ErrorKind::NotSupported
        );
    }

    #[test]
    fn device_message_rejects_length_mismatch_and_reserved_status() {
        assert!(DeviceMessage::decode(&[0x80, 1, 0, 0, 0, 0, 1, 0, 0, 0]).is_err());
        assert!(DeviceMessage::decode(&[0x80, 0, 0, 0, 0, 0, 1, 3, 0, 0]).is_err());
    }

    #[test]
    fn device_message_preserves_status_and_payload() {
        let decoded =
            DeviceMessage::decode(&[RDR_TO_PC_DATA_BLOCK, 2, 0, 0, 0, 0, 9, 0, 0, 0, 0x90, 0x00])
                .expect("decode");
        assert_eq!(decoded.icc_status, IccStatus::Active);
        assert_eq!(decoded.command_status, CommandStatus::Complete);
        assert_eq!(&*decoded.payload, [0x90, 0]);
    }

    #[test]
    fn class_descriptor_extracts_portable_facts() {
        let mut descriptor = [0_u8; 54];
        descriptor[0] = 54;
        descriptor[1] = 0x21;
        descriptor[6..10].copy_from_slice(&3_u32.to_le_bytes());
        descriptor[40..44].copy_from_slice(&0x0004_0000_u32.to_le_bytes());
        descriptor[44..48].copy_from_slice(&4096_u32.to_le_bytes());
        let facts = parse_class_descriptor(&descriptor).expect("parse");
        assert_eq!(facts.slots, 1);
        assert_eq!(facts.exchange, DescriptorExchange::ExtendedApdu);
        assert!(facts.supports_t0 && facts.supports_t1);
    }

    #[test]
    fn class_descriptor_rejects_identity_length_and_slot_overflow() {
        let mut descriptor = [0_u8; 54];
        descriptor[0] = 54;
        descriptor[1] = 0x21;
        descriptor[44..48].copy_from_slice(&10_u32.to_le_bytes());
        assert!(parse_class_descriptor(&descriptor).is_ok());

        descriptor[0] = 53;
        assert!(parse_class_descriptor(&descriptor).is_err());
        descriptor[0] = 54;
        descriptor[1] = 0x22;
        assert!(parse_class_descriptor(&descriptor).is_err());
        descriptor[1] = 0x21;
        descriptor[4] = u8::MAX;
        assert!(parse_class_descriptor(&descriptor).is_err());
        descriptor[4] = 0;
        descriptor[44..48].copy_from_slice(&9_u32.to_le_bytes());
        assert!(parse_class_descriptor(&descriptor).is_err());
        let mut trailing = descriptor.to_vec();
        trailing.push(0);
        assert!(parse_class_descriptor(&trailing).is_err());
    }

    #[test]
    fn response_validation_rejects_wrong_identity_failure_and_absence() {
        let good = DeviceMessage::decode(&[0x80, 0, 0, 0, 0, 0, 7, 0, 0, 0]).expect("decode");
        assert!(good.validate_for(0x80, 0, 7).is_ok());
        assert!(good.validate_for(0x81, 0, 7).is_err());
        assert!(good.validate_for(0x80, 1, 7).is_err());
        assert!(good.validate_for(0x80, 0, 8).is_err());

        let absent = DeviceMessage::decode(&[0x80, 0, 0, 0, 0, 0, 7, 2, 0, 0]).expect("decode");
        assert!(matches!(
            absent.validate_for(0x80, 0, 7),
            Err(error) if error.kind() == crate::ErrorKind::CardGone
        ));
        let failed = DeviceMessage::decode(&[0x80, 0, 0, 0, 0, 0, 7, 0x40, 5, 0]).expect("decode");
        assert!(failed.validate_for(0x80, 0, 7).is_err());
    }

    #[test]
    fn descriptor_exchange_and_protocol_bits_are_independent() {
        for (feature, expected) in [
            (0_u32, DescriptorExchange::Character),
            (0x0001_0000_u32, DescriptorExchange::Tpdu),
            (0x0002_0000_u32, DescriptorExchange::ShortApdu),
            (0x0004_0000_u32, DescriptorExchange::ExtendedApdu),
        ] {
            let mut descriptor = [0_u8; 54];
            descriptor[0] = 54;
            descriptor[1] = 0x21;
            descriptor[40..44].copy_from_slice(&(feature | 0x8000_0000).to_le_bytes());
            descriptor[44..48].copy_from_slice(&10_u32.to_le_bytes());
            let facts = parse_class_descriptor(&descriptor).expect("descriptor");
            assert_eq!(facts.exchange, expected);
        }

        for (protocols, t0, t1) in [
            (0_u32, false, false),
            (1_u32, true, false),
            (2_u32, false, true),
            (4_u32, false, false),
            (3_u32, true, true),
        ] {
            let mut descriptor = [0_u8; 54];
            descriptor[0] = 54;
            descriptor[1] = 0x21;
            descriptor[6..10].copy_from_slice(&protocols.to_le_bytes());
            descriptor[44..48].copy_from_slice(&10_u32.to_le_bytes());
            let facts = parse_class_descriptor(&descriptor).expect("descriptor");
            assert_eq!((facts.supports_t0, facts.supports_t1), (t0, t1));
        }
    }

    #[test]
    fn device_message_distinguishes_inactive_and_time_extension() {
        let inactive = DeviceMessage::decode(&[RDR_TO_PC_SLOT_STATUS, 0, 0, 0, 0, 0, 1, 1, 0, 0])
            .expect("inactive");
        assert_eq!(inactive.icc_status, IccStatus::Inactive);
        let extension =
            DeviceMessage::decode(&[0x80, 0, 0, 0, 0, 0, 1, 0x80, 1, 0]).expect("time extension");
        assert_eq!(extension.command_status, CommandStatus::TimeExtension);
    }
}
