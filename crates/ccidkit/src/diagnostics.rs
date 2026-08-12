// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Stable observation values for reader bring-up and reproducible bug reports.

use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::operation::{Completer, Operation};
use crate::{Error, ErrorKind, ReaderId};

/// A built-in route to a reader.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum BackendKind {
    /// Direct USB CCID through `nusb`.
    NativeUsb,
    /// The platform PC/SC service.
    Pcsc,
    /// A deterministic in-memory reader from [`crate::testing`].
    Virtual,
}

/// The unit a reader accepts in its transfer command.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ExchangeLevel {
    /// Complete short APDUs.
    ShortApdu,
    /// Complete short and extended APDUs.
    ExtendedApdu,
    /// Transport protocol data units; the native adapter drives negotiated T=1/LRC.
    Tpdu,
    /// Individual card-protocol characters, which v1 deliberately rejects.
    Character,
}

/// Portable facts learned from a reader without exposing backend descriptor types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capabilities {
    slots: u8,
    maximum_message_length: u32,
    exchange_level: ExchangeLevel,
    supports_t0: bool,
    supports_t1: bool,
}

impl Capabilities {
    pub(crate) const fn new(
        slots: u8,
        maximum_message_length: u32,
        exchange_level: ExchangeLevel,
        supports_t0: bool,
        supports_t1: bool,
    ) -> Self {
        Self {
            slots,
            maximum_message_length,
            exchange_level,
            supports_t0,
            supports_t1,
        }
    }

    /// Number of independently addressable card slots.
    #[must_use]
    pub const fn slots(&self) -> u8 {
        self.slots
    }

    /// Largest complete CCID or APDU message reported by the reader.
    #[must_use]
    pub const fn maximum_message_length(&self) -> u32 {
        self.maximum_message_length
    }

    /// Reader exchange granularity.
    #[must_use]
    pub const fn exchange_level(&self) -> ExchangeLevel {
        self.exchange_level
    }

    /// Whether protocol T=0 is supported.
    #[must_use]
    pub const fn supports_t0(&self) -> bool {
        self.supports_t0
    }

    /// Whether protocol T=1 is supported.
    #[must_use]
    pub const fn supports_t1(&self) -> bool {
        self.supports_t1
    }
}

/// The protocol layer at which a trace frame was observed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Layer {
    /// Command or response APDU.
    Apdu,
    /// Complete CCID bulk message.
    Ccid,
}

/// Direction relative to the host application.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Direction {
    /// Host to reader or card.
    Out,
    /// Reader or card to host.
    In,
}

/// One immutable, timestamp-free protocol frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFrame {
    layer: Layer,
    direction: Direction,
    backend: BackendKind,
    reader: ReaderId,
    bytes: Arc<[u8]>,
}

impl TraceFrame {
    pub(crate) fn new(
        layer: Layer,
        direction: Direction,
        backend: BackendKind,
        reader: ReaderId,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Self {
        Self {
            layer,
            direction,
            backend,
            reader,
            bytes: bytes.into(),
        }
    }

    /// Layer at which the bytes were captured.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        self.layer
    }

    /// Direction relative to the host.
    #[must_use]
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    /// Backend that produced the frame.
    #[must_use]
    pub const fn backend(&self) -> BackendKind {
        self.backend
    }

    /// Reader associated with the frame.
    #[must_use]
    pub const fn reader(&self) -> ReaderId {
        self.reader
    }

    /// Exact frame bytes. APDU traces may contain secrets and are opt-in.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One item from a bounded trace subscription.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TraceEvent {
    /// A captured protocol frame.
    Frame(TraceFrame),
    /// Frames were dropped because this subscriber did not drain fast enough.
    Overflow {
        /// Number of frames dropped since the previous delivered event.
        dropped: u64,
    },
}

struct Subscriber {
    sender: SyncSender<TraceEvent>,
    dropped: u64,
}

/// Internal broadcast point shared by a context and its cards.
#[derive(Default)]
pub(crate) struct TraceHub {
    subscribers: Mutex<Vec<Subscriber>>,
}

impl TraceHub {
    fn lock(&self) -> MutexGuard<'_, Vec<Subscriber>> {
        match self.subscribers.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub(crate) fn subscribe(self: &Arc<Self>) -> Trace {
        let (sender, receiver) = mpsc::sync_channel(128);
        self.lock().push(Subscriber { sender, dropped: 0 });
        Trace {
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub(crate) fn emit(&self, event: &TraceEvent) {
        self.lock().retain_mut(|subscriber| {
            if subscriber.dropped != 0 {
                match subscriber.sender.try_send(TraceEvent::Overflow {
                    dropped: subscriber.dropped,
                }) {
                    Ok(()) => subscriber.dropped = 0,
                    Err(TrySendError::Full(_)) => return true,
                    Err(TrySendError::Disconnected(_)) => return false,
                }
            }
            match subscriber.sender.try_send(event.clone()) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => {
                    subscriber.dropped = subscriber.dropped.saturating_add(1);
                    true
                },
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }
}

/// An opt-in, bounded subscription to protocol frames.
///
/// Captured APDUs can contain PINs or other secrets. Create a trace only when the
/// resulting bytes will be handled as sensitive diagnostic material.
pub struct Trace {
    receiver: Arc<Mutex<Receiver<TraceEvent>>>,
}

impl Trace {
    /// Wait for the next frame or overflow notification.
    pub fn next_event(&mut self) -> Operation<'_, TraceEvent> {
        let (operation, completer) = Operation::pending();
        let receiver = Arc::clone(&self.receiver);
        let spawn = std::thread::Builder::new()
            .name("ccidkit-trace".to_owned())
            .spawn(move || receive_trace(&receiver, completer));
        if let Err(error) = spawn {
            return Operation::ready(Err(Error::with_source(
                ErrorKind::Transport,
                "failed to start the trace receiver",
                error,
            )));
        }
        operation
    }
}

impl std::fmt::Debug for Trace {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Trace").finish_non_exhaustive()
    }
}

fn receive_trace(receiver: &Mutex<Receiver<TraceEvent>>, completer: Completer<TraceEvent>) {
    loop {
        if completer.is_cancelled() {
            completer.complete(Err(Error::from_kind(ErrorKind::Cancelled)));
            return;
        }
        let outcome = match receiver.lock() {
            Ok(guard) => guard.recv_timeout(Duration::from_millis(50)),
            Err(poisoned) => poisoned
                .into_inner()
                .recv_timeout(Duration::from_millis(50)),
        };
        match outcome {
            Ok(event) => {
                completer.complete(Ok(event));
                return;
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {},
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                completer.complete(Err(Error::new(
                    ErrorKind::Cancelled,
                    "trace source was closed",
                )));
                return;
            },
        }
    }
}
