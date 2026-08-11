// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use nusb as api;

use api::descriptors::TransferType;
use api::transfer::{Buffer, Bulk, In, Out, TransferError};
use api::{Device, Endpoint, MaybeFuture};

use crate::backend::ReaderRecord;
use crate::backend::quirks::{self, Quirks};
use crate::ccid::{
    self, CommandStatus, DescriptorExchange, DescriptorFacts, DeviceMessage, RDR_TO_PC_DATA_BLOCK,
    RDR_TO_PC_PARAMETERS, RDR_TO_PC_SLOT_STATUS, TransportParameters,
};
use crate::diagnostics::{BackendKind, Capabilities, ExchangeLevel};
use crate::protocol::{T1Action, T1Machine};
use crate::{Atr, Command, Error, ErrorKind, ReaderId, Response, Result};

const CCID_CLASS: u8 = 0x0B;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(30);
const SLOW_TRANSFER_TIMEOUT: Duration = Duration::from_secs(60);
const SELECTOR_SEPARATOR: char = '\u{1f}';

pub(crate) fn probe() -> Result<()> {
    api::list_devices()
        .wait()
        .map(|_| ())
        .map_err(|error| map_error(error, "failed to enumerate USB devices"))
}

pub(crate) fn readers() -> Result<Vec<ReaderRecord>> {
    let devices = api::list_devices()
        .wait()
        .map_err(|error| map_error(error, "failed to enumerate USB devices"))?;
    let mut readers = Vec::new();
    for device in devices {
        for interface in device
            .interfaces()
            .filter(|interface| interface.class() == CCID_CLASS)
        {
            let interface_number = interface.interface_number();
            let name = device
                .product_string()
                .or_else(|| interface.interface_string())
                .map_or_else(
                    || {
                        format!(
                            "CCID {:04X}:{:04X}",
                            device.vendor_id(),
                            device.product_id()
                        )
                    },
                    str::to_owned,
                );
            let facts = device
                .open()
                .wait()
                .ok()
                .and_then(|opened| descriptor_facts(&opened, interface_number).ok())
                .map(|facts| {
                    apply_quirks(
                        facts,
                        quirks::lookup(device.vendor_id(), device.product_id()),
                    )
                });
            let slots = facts.map_or(1, |facts| facts.slots);
            for slot in 0..slots {
                let selector = selector(
                    device.bus_id(),
                    device.device_address(),
                    interface_number,
                    slot,
                );
                let slot_name = if slots == 1 {
                    name.clone()
                } else {
                    format!("{name} [slot {slot}]")
                };
                readers.push(ReaderRecord {
                    id: ReaderId::from_name(BackendKind::NativeUsb, &selector),
                    name: Arc::from(slot_name),
                    backend: BackendKind::NativeUsb,
                    capabilities: facts.map_or_else(fallback_capabilities, capabilities),
                    selector: Arc::from(selector),
                });
            }
        }
    }
    Ok(readers)
}

pub(crate) fn connect(selector: &str) -> Result<(UsbCard, Atr)> {
    let (bus, address, interface_number, slot) = parse_selector(selector)?;
    let devices = api::list_devices()
        .wait()
        .map_err(|error| map_error(error, "failed to enumerate USB devices"))?;
    let device_info = devices
        .into_iter()
        .find(|device| device.bus_id() == bus && device.device_address() == address)
        .ok_or_else(|| Error::from_kind(ErrorKind::NoReader))?;
    let quirks = quirks::lookup(device_info.vendor_id(), device_info.product_id());
    let device = device_info
        .open()
        .wait()
        .map_err(|error| map_error(error, "failed to open USB reader"))?;
    let (facts, alternate_setting, bulk_out, bulk_in) = interface_facts(&device, interface_number)?;
    let facts = apply_quirks(facts, quirks);
    if slot >= facts.slots {
        return Err(Error::new(
            ErrorKind::NoReader,
            "selected CCID slot is no longer reported by the reader",
        ));
    }
    if facts.exchange == DescriptorExchange::Character {
        return Err(Error::new(
            ErrorKind::NotSupported,
            "CCID character-level exchange readers are outside ccidkit's safe API",
        ));
    }
    let interface = device
        .detach_and_claim_interface(interface_number)
        .wait()
        .map_err(|error| map_error(error, "failed to claim CCID interface"))?;
    if interface.get_alt_setting() != alternate_setting {
        interface
            .set_alt_setting(alternate_setting)
            .wait()
            .map_err(|error| map_error(error, "failed to select CCID alternate setting"))?;
    }
    let output = interface
        .endpoint::<Bulk, Out>(bulk_out)
        .map_err(|error| map_error(error, "failed to open CCID bulk-out endpoint"))?;
    let input = interface
        .endpoint::<Bulk, In>(bulk_in)
        .map_err(|error| map_error(error, "failed to open CCID bulk-in endpoint"))?;
    let max_message_length = usize::try_from(facts.maximum_message_length).map_err(|_| {
        Error::new(
            ErrorKind::Protocol,
            "reader's maximum CCID message length does not fit this platform",
        )
    })?;
    let mut card = UsbCard {
        output,
        input,
        slot,
        sequence: 0,
        max_message_length,
        exchange_level: facts.exchange,
        protocol: CardProtocol::T0,
        ifsc: 32,
        quirks,
        transfer_timeout: if quirks.slow_power_on {
            SLOW_TRANSFER_TIMEOUT
        } else {
            TRANSFER_TIMEOUT
        },
    };
    let atr = card.activate()?;
    Ok((card, atr))
}

pub(crate) struct UsbCard {
    output: Endpoint<Bulk, Out>,
    input: Endpoint<Bulk, In>,
    slot: u8,
    sequence: u8,
    max_message_length: usize,
    exchange_level: DescriptorExchange,
    protocol: CardProtocol,
    ifsc: u8,
    quirks: Quirks,
    transfer_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CardProtocol {
    T0,
    T1,
}

impl UsbCard {
    pub(crate) fn transmit(&mut self, command: &Command) -> Result<Response> {
        if command.uses_extended_encoding()
            && (self.exchange_level == DescriptorExchange::ShortApdu
                || (self.exchange_level == DescriptorExchange::Tpdu
                    && self.protocol == CardProtocol::T0))
        {
            return Err(Error::new(
                ErrorKind::NotSupported,
                "reader exchange level cannot carry an extended APDU",
            ));
        }
        if self.exchange_level == DescriptorExchange::Tpdu {
            return match self.protocol {
                CardProtocol::T1 => self.transmit_t1(command),
                CardProtocol::T0 => Err(Error::new(
                    ErrorKind::NotSupported,
                    "native T=0 TPDU procedure-byte handling is not implemented",
                )),
            };
        }
        self.transmit_apdu_or_t0(command)
    }

    fn transmit_apdu_or_t0(&mut self, command: &Command) -> Result<Response> {
        let sequence = self.next_sequence();
        let request = ccid::transfer_block(self.slot, sequence, &command.to_bytes())?;
        let response = self.exchange(request, RDR_TO_PC_DATA_BLOCK, sequence)?;
        Response::from_bytes(&response.payload)
    }

    fn transmit_t1(&mut self, command: &Command) -> Result<Response> {
        let mut machine = T1Machine::new(command.to_bytes(), self.ifsc)?;
        let mut action = machine.start()?;
        for _ in 0..256 {
            action = match action {
                T1Action::Complete(bytes) => return Response::from_bytes(&bytes),
                T1Action::Send(block) => {
                    let sequence = self.next_sequence();
                    let request = ccid::transfer_block(self.slot, sequence, &block)?;
                    let response = self.exchange(request, RDR_TO_PC_DATA_BLOCK, sequence)?;
                    machine.accept(&response.payload)?
                },
            };
        }
        Err(Error::new(
            ErrorKind::Protocol,
            "T=1 exchange exceeded 256 transport actions",
        ))
    }

    pub(crate) fn reset(&mut self) -> Result<Atr> {
        self.power_off()?;
        self.activate()
    }

    fn activate(&mut self) -> Result<Atr> {
        let atr = self.power_on()?;
        if self.exchange_level == DescriptorExchange::Tpdu {
            let parameters = self.parameters()?;
            match parameters {
                TransportParameters::T0 => {
                    self.protocol = CardProtocol::T0;
                },
                TransportParameters::T1 {
                    ifsc,
                    uses_crc: false,
                } => {
                    self.protocol = CardProtocol::T1;
                    self.ifsc = ifsc;
                },
                TransportParameters::T1 { uses_crc: true, .. } => {
                    return Err(Error::new(
                        ErrorKind::NotSupported,
                        "reader negotiated T=1 CRC, which this native transport does not implement",
                    ));
                },
            }
        }
        Ok(atr)
    }

    fn power_on(&mut self) -> Result<Atr> {
        let sequence = self.next_sequence();
        let response = self.exchange(
            ccid::power_on(self.slot, sequence),
            RDR_TO_PC_DATA_BLOCK,
            sequence,
        )?;
        Atr::parse(&response.payload).map_err(|error| {
            Error::new(
                ErrorKind::Protocol,
                format!("reader returned an invalid ATR: {error}"),
            )
        })
    }

    fn power_off(&mut self) -> Result<()> {
        let sequence = self.next_sequence();
        self.exchange(
            ccid::power_off(self.slot, sequence),
            RDR_TO_PC_SLOT_STATUS,
            sequence,
        )
        .map(|_| ())
    }

    fn parameters(&mut self) -> Result<TransportParameters> {
        let sequence = self.next_sequence();
        self.exchange(
            ccid::get_parameters(self.slot, sequence),
            RDR_TO_PC_PARAMETERS,
            sequence,
        )?
        .transport_parameters()
    }

    fn exchange(
        &mut self,
        request: Vec<u8>,
        expected_type: u8,
        sequence: u8,
    ) -> Result<DeviceMessage> {
        let request_length = request.len();
        let completion = self
            .output
            .transfer_blocking(request.into(), self.transfer_timeout);
        completion
            .status
            .map_err(|error| map_transfer(error, "CCID bulk-out transfer failed"))?;
        let packet_size = self.output.max_packet_size();
        if self.quirks.needs_zlp && request_length.checked_rem(packet_size) == Some(0) {
            let zlp = self
                .output
                .transfer_blocking(Vec::<u8>::new().into(), self.transfer_timeout);
            zlp.status
                .map_err(|error| map_transfer(error, "CCID zero-length packet failed"))?;
        }

        for _ in 0..32 {
            let message = DeviceMessage::decode(&self.read_message()?)?;
            message.validate_for(expected_type, self.slot, sequence)?;
            if message.command_status != CommandStatus::TimeExtension {
                return Ok(message);
            }
        }
        Err(Error::new(
            ErrorKind::Timeout,
            "reader requested more than 32 consecutive CCID time extensions",
        ))
    }

    fn read_message(&mut self) -> Result<Vec<u8>> {
        let packet = self.input.max_packet_size();
        let requested = self
            .max_message_length
            .max(10)
            .checked_add(packet.saturating_sub(1))
            .and_then(|length| length.checked_div(packet))
            .and_then(|packets| packets.checked_mul(packet))
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "CCID input buffer overflows"))?;
        let completion = self
            .input
            .transfer_blocking(Buffer::new(requested), self.transfer_timeout);
        completion
            .status
            .map_err(|error| map_transfer(error, "CCID bulk-in transfer failed"))?;
        completion
            .buffer
            .get(..completion.actual_len)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| Error::new(ErrorKind::Transport, "USB completion length is invalid"))
    }

    fn next_sequence(&mut self) -> u8 {
        let current = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        current
    }
}

fn interface_facts(device: &Device, interface_number: u8) -> Result<(DescriptorFacts, u8, u8, u8)> {
    let configuration = device.active_configuration().map_err(|error| {
        Error::new(
            ErrorKind::Protocol,
            format!("USB reader has no active configuration: {error}"),
        )
    })?;
    if let Some(descriptor) = configuration.interface_alt_settings().find(|descriptor| {
        descriptor.interface_number() == interface_number && descriptor.class() == CCID_CLASS
    }) {
        let class = descriptor
            .descriptors()
            .find(|item| item.descriptor_type() == 0x21)
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "CCID class descriptor is missing"))?;
        let facts = ccid::parse_class_descriptor(&class)?;
        let mut bulk_in = None;
        let mut bulk_out = None;
        for endpoint in descriptor
            .endpoints()
            .filter(|endpoint| endpoint.transfer_type() == TransferType::Bulk)
        {
            if endpoint.address() & 0x80 == 0 {
                bulk_out = Some(endpoint.address());
            } else {
                bulk_in = Some(endpoint.address());
            }
        }
        let bulk_out = bulk_out
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "CCID bulk-out endpoint is missing"))?;
        let bulk_in = bulk_in
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "CCID bulk-in endpoint is missing"))?;
        return Ok((facts, descriptor.alternate_setting(), bulk_out, bulk_in));
    }
    Err(Error::new(
        ErrorKind::Protocol,
        "selected USB interface has no CCID alternate setting",
    ))
}

fn descriptor_facts(device: &Device, interface_number: u8) -> Result<DescriptorFacts> {
    interface_facts(device, interface_number).map(|(facts, _, _, _)| facts)
}

fn apply_quirks(mut facts: DescriptorFacts, quirks: Quirks) -> DescriptorFacts {
    if quirks.force_short_apdu && facts.exchange == DescriptorExchange::ExtendedApdu {
        facts.exchange = DescriptorExchange::ShortApdu;
    }
    facts
}

fn capabilities(facts: DescriptorFacts) -> Capabilities {
    let exchange = match facts.exchange {
        DescriptorExchange::Character => ExchangeLevel::Character,
        DescriptorExchange::Tpdu => ExchangeLevel::Tpdu,
        DescriptorExchange::ShortApdu => ExchangeLevel::ShortApdu,
        DescriptorExchange::ExtendedApdu => ExchangeLevel::ExtendedApdu,
    };
    Capabilities::new(
        facts.slots,
        facts.maximum_message_length,
        exchange,
        facts.supports_t0,
        facts.supports_t1,
    )
}

const fn fallback_capabilities() -> Capabilities {
    Capabilities::new(1, 271, ExchangeLevel::ShortApdu, true, true)
}

fn selector(bus: &str, address: u8, interface: u8, slot: u8) -> String {
    format!(
        "{bus}{SELECTOR_SEPARATOR}{address}{SELECTOR_SEPARATOR}{interface}{SELECTOR_SEPARATOR}{slot}"
    )
}

fn parse_selector(selector: &str) -> Result<(&str, u8, u8, u8)> {
    let mut fields = selector.split(SELECTOR_SEPARATOR);
    let bus = fields.next().unwrap_or_default();
    let address = fields
        .next()
        .and_then(|field| field.parse().ok())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid USB reader selector"))?;
    let interface = fields
        .next()
        .and_then(|field| field.parse().ok())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid USB reader selector"))?;
    let slot = fields
        .next()
        .and_then(|field| field.parse().ok())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid USB reader selector"))?;
    if bus.is_empty() || fields.next().is_some() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "invalid USB reader selector",
        ));
    }
    Ok((bus, address, interface, slot))
}

fn map_error(error: api::Error, context: &str) -> Error {
    let kind = match error.kind() {
        api::ErrorKind::PermissionDenied => ErrorKind::PermissionDenied,
        api::ErrorKind::NotFound => ErrorKind::NoReader,
        api::ErrorKind::Unsupported => ErrorKind::NotSupported,
        api::ErrorKind::Busy => ErrorKind::Busy,
        _ => ErrorKind::Transport,
    };
    Error::with_source(kind, format!("{context}: {error}"), error)
}

fn map_transfer(error: TransferError, context: &str) -> Error {
    let kind = match error {
        TransferError::Cancelled => ErrorKind::Timeout,
        TransferError::Disconnected => ErrorKind::CardGone,
        TransferError::InvalidArgument => ErrorKind::InvalidInput,
        TransferError::Stall | TransferError::Fault | TransferError::Unknown(_) => {
            ErrorKind::Transport
        },
    };
    Error::with_source(kind, format!("{context}: {error}"), error)
}

#[cfg(test)]
mod tests {
    use super::{parse_selector, selector};

    #[test]
    fn internal_usb_selector_round_trips_opaque_bus_names() {
        let encoded = selector("pci:0000:00", 7, 2, 3);
        assert!(matches!(
            parse_selector(&encoded),
            Ok(("pci:0000:00", 7, 2, 3))
        ));
    }
}
