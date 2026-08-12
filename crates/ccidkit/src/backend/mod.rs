// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

mod virtual_reader;

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
mod quirks {
    include!(concat!(env!("OUT_DIR"), "/ccidkit_quirks.rs"));
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
))]
mod pcsc_backend;

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
mod usb_backend;

use std::sync::{Arc, Mutex};

use crate::diagnostics::{BackendKind, Capabilities};
use crate::testing::Scenario;
use crate::{Atr, Command, ReaderId, Response, Result};

pub(crate) use virtual_reader::{VirtualCard, VirtualEvent, VirtualState};

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
))]
use pcsc_backend::{PcscCard, PcscTransaction};

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
use usb_backend::UsbCard;

#[derive(Clone, Debug)]
pub(crate) struct ReaderRecord {
    pub(crate) id: ReaderId,
    pub(crate) name: Arc<str>,
    pub(crate) backend: BackendKind,
    pub(crate) capabilities: Capabilities,
    pub(crate) selector: Arc<str>,
}

#[derive(Clone, Debug)]
pub(crate) enum Factory {
    Virtual(Arc<Mutex<VirtualState>>),
    Pcsc,
    NativeUsb,
}

impl Factory {
    pub(crate) fn virtual_reader(scenario: Scenario) -> Self {
        Self::Virtual(Arc::new(Mutex::new(VirtualState::new(scenario))))
    }

    pub(crate) const fn kind(&self) -> BackendKind {
        match self {
            Self::Virtual(_) => BackendKind::Virtual,
            Self::Pcsc => BackendKind::Pcsc,
            Self::NativeUsb => BackendKind::NativeUsb,
        }
    }

    pub(crate) fn probe(&self) -> Result<()> {
        match self {
            Self::Virtual(_) => Ok(()),
            Self::Pcsc => pcsc_probe(),
            Self::NativeUsb => usb_probe(),
        }
    }

    pub(crate) fn readers(&self) -> Result<Vec<ReaderRecord>> {
        match self {
            Self::Virtual(state) => virtual_reader::readers(state),
            Self::Pcsc => pcsc_readers(),
            Self::NativeUsb => usb_readers(),
        }
    }

    pub(crate) fn connect(&self, reader: &ReaderRecord) -> Result<(CardIo, Atr)> {
        match self {
            Self::Virtual(state) => {
                let (card, atr) = virtual_reader::connect(state)?;
                Ok((CardIo::Virtual(card), atr))
            },
            Self::Pcsc => pcsc_connect(&reader.selector),
            Self::NativeUsb => usb_connect(&reader.selector),
        }
    }

    pub(crate) fn virtual_event(&self) -> Option<virtual_reader::VirtualEvent> {
        match self {
            Self::Virtual(state) => virtual_reader::next_event(state),
            Self::Pcsc | Self::NativeUsb => None,
        }
    }
}

pub(crate) enum CardIo {
    Virtual(VirtualCard),
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", feature = "pcsc")
    ))]
    Pcsc(PcscCard),
    #[cfg(any(
        target_os = "linux",
        all(target_os = "windows", feature = "native-usb")
    ))]
    Usb(UsbCard),
}

pub(crate) enum CardTransaction<'a> {
    Virtual(&'a mut VirtualCard),
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", feature = "pcsc")
    ))]
    Pcsc(PcscTransaction<'a>),
    #[cfg(any(
        target_os = "linux",
        all(target_os = "windows", feature = "native-usb")
    ))]
    Usb(&'a mut UsbCard),
}

trait BeginTransaction {
    type Transaction<'a>
    where
        Self: 'a;

    fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>>;
}

impl BeginTransaction for VirtualCard {
    type Transaction<'a> = &'a mut VirtualCard;

    fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>> {
        Ok(self)
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
))]
impl BeginTransaction for PcscCard {
    type Transaction<'a> = PcscTransaction<'a>;

    fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>> {
        self.transaction()
    }
}

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
impl BeginTransaction for UsbCard {
    type Transaction<'a> = &'a mut UsbCard;

    fn begin_transaction(&mut self) -> Result<Self::Transaction<'_>> {
        Ok(self)
    }
}

impl CardIo {
    pub(crate) fn transaction(&mut self) -> Result<CardTransaction<'_>> {
        match self {
            Self::Virtual(card) => card.begin_transaction().map(CardTransaction::Virtual),
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                all(target_os = "linux", feature = "pcsc")
            ))]
            Self::Pcsc(card) => card.begin_transaction().map(CardTransaction::Pcsc),
            #[cfg(any(
                target_os = "linux",
                all(target_os = "windows", feature = "native-usb")
            ))]
            Self::Usb(card) => card.begin_transaction().map(CardTransaction::Usb),
        }
    }

    pub(crate) fn transmit_raw(
        &mut self,
        command: &Command,
        cancelled: impl Fn() -> bool,
    ) -> Result<Response> {
        match self {
            Self::Virtual(card) => card.transmit(command, cancelled),
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                all(target_os = "linux", feature = "pcsc")
            ))]
            Self::Pcsc(card) => card.transmit(command),
            #[cfg(any(
                target_os = "linux",
                all(target_os = "windows", feature = "native-usb")
            ))]
            Self::Usb(card) => card.transmit(command),
        }
    }

    pub(crate) fn reset(&mut self) -> Result<Atr> {
        match self {
            Self::Virtual(card) => card.reset(),
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                all(target_os = "linux", feature = "pcsc")
            ))]
            Self::Pcsc(card) => card.reset(),
            #[cfg(any(
                target_os = "linux",
                all(target_os = "windows", feature = "native-usb")
            ))]
            Self::Usb(card) => card.reset(),
        }
    }
}

impl CardTransaction<'_> {
    pub(crate) fn transmit_raw(
        &mut self,
        command: &Command,
        cancelled: impl Fn() -> bool,
    ) -> Result<Response> {
        match self {
            Self::Virtual(card) => card.transmit(command, cancelled),
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                all(target_os = "linux", feature = "pcsc")
            ))]
            Self::Pcsc(transaction) => transaction.transmit(command),
            #[cfg(any(
                target_os = "linux",
                all(target_os = "windows", feature = "native-usb")
            ))]
            Self::Usb(card) => card.transmit(command),
        }
    }

    pub(crate) fn reset(&mut self) -> Result<Atr> {
        match self {
            Self::Virtual(card) => card.reset(),
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                all(target_os = "linux", feature = "pcsc")
            ))]
            Self::Pcsc(transaction) => transaction.reset(),
            #[cfg(any(
                target_os = "linux",
                all(target_os = "windows", feature = "native-usb")
            ))]
            Self::Usb(card) => card.reset(),
        }
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
))]
fn pcsc_probe() -> Result<()> {
    pcsc_backend::probe()
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
)))]
fn pcsc_probe() -> Result<()> {
    Err(crate::Error::new(
        crate::ErrorKind::BackendUnavailable,
        "PC/SC is not enabled; on Linux build with feature `pcsc`",
    ))
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
))]
fn pcsc_readers() -> Result<Vec<ReaderRecord>> {
    pcsc_backend::readers()
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
)))]
fn pcsc_readers() -> Result<Vec<ReaderRecord>> {
    pcsc_probe().map(|()| Vec::new())
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
))]
fn pcsc_connect(selector: &str) -> Result<(CardIo, Atr)> {
    let (card, atr) = pcsc_backend::connect(selector)?;
    Ok((CardIo::Pcsc(card), atr))
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", feature = "pcsc")
)))]
fn pcsc_connect(_selector: &str) -> Result<(CardIo, Atr)> {
    Err(crate::Error::from_kind(
        crate::ErrorKind::BackendUnavailable,
    ))
}

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
fn usb_probe() -> Result<()> {
    usb_backend::probe()
}

#[cfg(not(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
)))]
fn usb_probe() -> Result<()> {
    Err(crate::Error::new(
        crate::ErrorKind::BackendUnavailable,
        "native USB is unavailable; Windows requires feature `native-usb` and WinUSB",
    ))
}

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
fn usb_readers() -> Result<Vec<ReaderRecord>> {
    usb_backend::readers()
}

#[cfg(not(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
)))]
fn usb_readers() -> Result<Vec<ReaderRecord>> {
    usb_probe().map(|()| Vec::new())
}

#[cfg(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
fn usb_connect(selector: &str) -> Result<(CardIo, Atr)> {
    let (card, atr) = usb_backend::connect(selector)?;
    Ok((CardIo::Usb(card), atr))
}

#[cfg(not(any(
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
)))]
fn usb_connect(_selector: &str) -> Result<(CardIo, Atr)> {
    Err(crate::Error::from_kind(
        crate::ErrorKind::BackendUnavailable,
    ))
}
