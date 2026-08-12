// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::CString;
use std::sync::Arc;

use pcsc as api;

use crate::backend::ReaderRecord;
use crate::diagnostics::{BackendKind, Capabilities, ExchangeLevel};
use crate::{Atr, Command, Error, ErrorKind, ReaderId, Response, Result};

pub(crate) fn probe() -> Result<()> {
    api::Context::establish(api::Scope::User)
        .map(|_| ())
        .map_err(|error| map_error(error, "failed to establish the PC/SC context"))
}

pub(crate) fn readers() -> Result<Vec<ReaderRecord>> {
    let context = api::Context::establish(api::Scope::User)
        .map_err(|error| map_error(error, "failed to establish the PC/SC context"))?;
    let names = context
        .list_readers_owned()
        .map_err(|error| map_error(error, "failed to enumerate PC/SC readers"))?;
    Ok(names
        .into_iter()
        .map(|name| {
            let name: Arc<str> = Arc::from(name.to_string_lossy().as_ref());
            ReaderRecord {
                id: ReaderId::from_name(BackendKind::Pcsc, &name),
                name: Arc::clone(&name),
                backend: BackendKind::Pcsc,
                capabilities: capabilities(),
                selector: name,
            }
        })
        .collect())
}

const fn capabilities() -> Capabilities {
    Capabilities::new(1, 65_538, ExchangeLevel::ExtendedApdu, true, true)
}

pub(crate) fn connect(selector: &str) -> Result<(PcscCard, Atr)> {
    let name = CString::new(selector).map_err(|error| {
        Error::with_source(
            ErrorKind::InvalidInput,
            "PC/SC reader name contains an interior NUL",
            error,
        )
    })?;
    let context = api::Context::establish(api::Scope::User)
        .map_err(|error| map_error(error, "failed to establish the PC/SC context"))?;
    let card = context
        .connect(&name, api::ShareMode::Shared, api::Protocols::ANY)
        .map_err(|error| map_error(error, "failed to connect to the card through PC/SC"))?;
    let atr = card
        .status2_owned()
        .map_err(|error| map_error(error, "failed to read the card ATR"))?;
    let atr = Atr::parse(atr.atr()).map_err(|error| {
        Error::new(
            ErrorKind::Protocol,
            format!("PC/SC returned an invalid ATR: {error}"),
        )
    })?;
    Ok((PcscCard { card }, atr))
}

pub(crate) struct PcscCard {
    card: api::Card,
}

impl PcscCard {
    pub(crate) fn transmit(&mut self, command: &Command) -> Result<Response> {
        transmit(&self.card, command)
    }

    pub(crate) fn reset(&mut self) -> Result<Atr> {
        self.card
            .reconnect(
                api::ShareMode::Shared,
                api::Protocols::ANY,
                api::Disposition::ResetCard,
            )
            .map_err(|error| map_error(error, "failed to reset the PC/SC card"))?;
        read_atr(&self.card, "failed to read the reset ATR")
    }

    pub(crate) fn transaction(&mut self) -> Result<PcscTransaction<'_>> {
        self.card
            .transaction()
            .map(|transaction| PcscTransaction { transaction })
            .map_err(|error| map_error(error, "failed to begin the PC/SC transaction"))
    }
}

pub(crate) struct PcscTransaction<'a> {
    transaction: api::Transaction<'a>,
}

impl PcscTransaction<'_> {
    pub(crate) fn transmit(&mut self, command: &Command) -> Result<Response> {
        transmit(&self.transaction, command)
    }

    pub(crate) fn reset(&mut self) -> Result<Atr> {
        self.transaction
            .reconnect(
                api::ShareMode::Shared,
                api::Protocols::ANY,
                api::Disposition::ResetCard,
            )
            .map_err(|error| map_error(error, "failed to reset the PC/SC card"))?;
        read_atr(&self.transaction, "failed to read the reset ATR")
    }
}

fn transmit(card: &api::Card, command: &Command) -> Result<Response> {
    let command = command.to_bytes();
    let mut response = vec![0_u8; api::MAX_BUFFER_SIZE_EXTENDED];
    let response = card
        .transmit(&command, &mut response)
        .map_err(|error| map_error(error, "PC/SC APDU exchange failed"))?;
    Response::from_bytes(response).map_err(|error| {
        Error::new(
            ErrorKind::Protocol,
            format!("PC/SC returned a malformed APDU response: {error}"),
        )
    })
}

fn read_atr(card: &api::Card, context: &str) -> Result<Atr> {
    let status = card
        .status2_owned()
        .map_err(|error| map_error(error, context))?;
    Atr::parse(status.atr()).map_err(|error| {
        Error::new(
            ErrorKind::Protocol,
            format!("PC/SC returned an invalid ATR: {error}"),
        )
    })
}

fn map_error(error: api::Error, context: &str) -> Error {
    let kind = match error {
        api::Error::NoReadersAvailable | api::Error::UnknownReader => ErrorKind::NoReader,
        api::Error::NoSmartcard | api::Error::UnpoweredCard => ErrorKind::CardAbsent,
        api::Error::RemovedCard | api::Error::ResetCard => ErrorKind::CardGone,
        api::Error::SharingViolation => ErrorKind::Busy,
        api::Error::Timeout | api::Error::WaitedTooLong => ErrorKind::Timeout,
        api::Error::Cancelled | api::Error::CancelledByUser | api::Error::SystemCancelled => {
            ErrorKind::Cancelled
        },
        api::Error::NoService | api::Error::ServiceStopped => ErrorKind::BackendUnavailable,
        api::Error::NoAccess | api::Error::SecurityViolation => ErrorKind::PermissionDenied,
        api::Error::UnsupportedFeature
        | api::Error::ReaderUnsupported
        | api::Error::CardUnsupported
        | api::Error::UnsupportedCard => ErrorKind::NotSupported,
        api::Error::InvalidAtr | api::Error::ProtoMismatch => ErrorKind::Protocol,
        _ => ErrorKind::Transport,
    };
    Error::with_source(kind, format!("{context}: {error}"), error)
}
