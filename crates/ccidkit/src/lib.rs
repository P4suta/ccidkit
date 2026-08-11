// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Smart-card I/O behind one small, runtime-neutral API.
//!
//! The ordinary path is deliberately short:
//!
//! ```no_run
//! let mut card = ccidkit::open_first().wait()?;
//! let command = ccidkit::Command::new(0x00, 0x84, 0x00, 0x00)
//!     .with_expected_len(8)?;
//! let response = card.transmit(command).wait()?;
//! # Ok::<(), ccidkit::Error>(())
//! ```
//!
//! Every I/O method returns [`Operation`], which can either be awaited or blocked on.
//! Backend handles and third-party types never cross the public boundary.

#![forbid(unsafe_code)]

mod backend;
#[cfg(any(
    test,
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
mod ccid;
mod diagnostics;
mod error;
mod facade;
mod model;
mod operation;
#[cfg(any(
    test,
    target_os = "linux",
    all(target_os = "windows", feature = "native-usb")
))]
mod protocol;

pub mod testing;

pub use diagnostics::{
    BackendKind, Capabilities, Direction, ExchangeLevel, Layer, Trace, TraceEvent, TraceFrame,
};
pub use error::{Error, ErrorKind, Result};
pub use facade::{
    Card, Context, ContextBuilder, Event, Monitor, Reader, ReaderId, Transaction, open_first,
};
pub use model::{Atr, Command, Response, StatusError, StatusWord};
pub use operation::Operation;
