// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use crate::backend::{CardIo, CardTransaction, Factory, ReaderRecord};
use crate::diagnostics::{
    BackendKind, Capabilities, Direction, Layer, Trace, TraceEvent, TraceFrame, TraceHub,
};
use crate::operation::{Completer, Operation};
use crate::testing::Scenario;
use crate::{Atr, Command, Error, ErrorKind, Response, Result};

/// A stable identifier within one machine and backend configuration.
///
/// The representation is deliberately opaque: applications may compare, sort, log,
/// and store it, but cannot couple themselves to a backend's device handles.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReaderId(u64);

impl ReaderId {
    pub(crate) fn from_name(backend: BackendKind, name: &str) -> Self {
        // FNV-1a is intentionally simple and deterministic. This is an identity token,
        // not a security boundary.
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let tag = match backend {
            BackendKind::NativeUsb => 1,
            BackendKind::Pcsc => 2,
            BackendKind::Virtual => 3,
        };
        hash ^= tag;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        for byte in name.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
        Self(hash)
    }
}

impl fmt::Debug for ReaderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ReaderId({self})")
    }
}

impl fmt::Display for ReaderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:016x}", self.0)
    }
}

struct ContextInner {
    factory: Factory,
    trace: Arc<TraceHub>,
}

/// An isolated entry point to reader discovery and diagnostics.
#[derive(Clone)]
pub struct Context {
    inner: Arc<ContextInner>,
}

impl Context {
    /// Open the platform-preferred built-in backend.
    ///
    /// Linux uses direct USB. Windows and macOS use the platform PC/SC service.
    pub fn new() -> Operation<'static, Self> {
        ContextBuilder::new().open()
    }

    /// Configure backend selection without exposing a backend implementation object.
    #[must_use]
    pub const fn builder() -> ContextBuilder {
        ContextBuilder::new()
    }

    pub(crate) fn from_scenario(scenario: Scenario) -> Operation<'static, Self> {
        open_factory(Factory::virtual_reader(scenario))
    }

    /// Enumerate the readers visible to this context.
    pub fn readers(&self) -> Operation<'static, Vec<Reader>> {
        let inner = Arc::clone(&self.inner);
        spawn_operation("ccidkit-readers", move |_| {
            inner
                .factory
                .readers()?
                .into_iter()
                .map(|record| Ok(Reader::new(Arc::clone(&inner), record)))
                .collect()
        })
    }

    /// Connect to the first enumerated card.
    pub fn open_first(&self) -> Operation<'static, Card> {
        let inner = Arc::clone(&self.inner);
        spawn_operation("ccidkit-open-first", move |_| {
            let record = inner
                .factory
                .readers()?
                .into_iter()
                .next()
                .ok_or_else(|| Error::from_kind(ErrorKind::NoReader))?;
            connect_card(&inner, &record)
        })
    }

    /// Observe hot-plug and scripted card-presence changes.
    pub fn monitor(&self) -> Result<Monitor> {
        let initial = self
            .inner
            .factory
            .readers()?
            .into_iter()
            .map(|record| (record.id, record))
            .collect();
        Ok(Monitor {
            inner: Arc::clone(&self.inner),
            state: Arc::new(Mutex::new(MonitorState { known: initial })),
        })
    }

    /// Subscribe to bounded, opt-in APDU and transport tracing.
    #[must_use]
    pub fn trace(&self) -> Trace {
        self.inner.trace.subscribe()
    }

    /// The selected built-in backend.
    #[must_use]
    pub fn backend(&self) -> BackendKind {
        self.inner.factory.kind()
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Context")
            .field("backend", &self.backend())
            .finish_non_exhaustive()
    }
}

/// The intentionally small configuration surface for opening a [`Context`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ContextBuilder {
    backend: Option<BackendKind>,
}

impl ContextBuilder {
    /// Start with the platform-preferred backend.
    #[must_use]
    pub const fn new() -> Self {
        Self { backend: None }
    }

    /// Select one built-in backend explicitly.
    #[must_use]
    pub const fn backend(mut self, backend: BackendKind) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Open the context.
    pub fn open(self) -> Operation<'static, Context> {
        let factory = match self.backend.unwrap_or_else(platform_default) {
            BackendKind::NativeUsb => Factory::NativeUsb,
            BackendKind::Pcsc => Factory::Pcsc,
            BackendKind::Virtual => {
                return Operation::ready(Err(Error::new(
                    ErrorKind::InvalidInput,
                    "virtual contexts are created with ccidkit::testing::open",
                )));
            },
        };
        open_factory(factory)
    }
}

const fn platform_default() -> BackendKind {
    if cfg!(target_os = "linux") {
        BackendKind::NativeUsb
    } else {
        BackendKind::Pcsc
    }
}

fn open_factory(factory: Factory) -> Operation<'static, Context> {
    spawn_operation("ccidkit-context", move |_| {
        factory.probe()?;
        Ok(Context {
            inner: Arc::new(ContextInner {
                factory,
                trace: Arc::new(TraceHub::default()),
            }),
        })
    })
}

/// Open the first card using the platform-preferred backend.
pub fn open_first() -> Operation<'static, Card> {
    spawn_operation("ccidkit-open-first", move |_| {
        let context = Context::new().wait()?;
        context.open_first().wait()
    })
}

/// One discovered reader.
#[derive(Clone)]
pub struct Reader {
    inner: Arc<ContextInner>,
    record: ReaderRecord,
}

impl Reader {
    fn new(inner: Arc<ContextInner>, record: ReaderRecord) -> Self {
        Self { inner, record }
    }

    /// Backend-independent reader identity.
    #[must_use]
    pub const fn id(&self) -> ReaderId {
        self.record.id
    }

    /// Human-readable reader name supplied by the backend.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.record.name
    }

    /// Built-in backend that owns this reader.
    #[must_use]
    pub const fn backend(&self) -> BackendKind {
        self.record.backend
    }

    /// Portable reader capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &Capabilities {
        &self.record.capabilities
    }

    /// Connect to the card currently present in this reader.
    pub fn connect(&self) -> Operation<'static, Card> {
        let inner = Arc::clone(&self.inner);
        let record = self.record.clone();
        spawn_operation("ccidkit-connect", move |_| connect_card(&inner, &record))
    }
}

impl fmt::Debug for Reader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Reader")
            .field("id", &self.id())
            .field("name", &self.name())
            .field("backend", &self.backend())
            .field("capabilities", &self.capabilities())
            .finish()
    }
}

fn connect_card(inner: &Arc<ContextInner>, record: &ReaderRecord) -> Result<Card> {
    let (io, atr) = inner.factory.connect(record)?;
    let (sender, receiver) = mpsc::channel();
    let worker_trace = Arc::clone(&inner.trace);
    let worker_record = record.clone();
    thread::Builder::new()
        .name(format!("ccidkit-card-{}", record.id))
        .spawn(move || card_worker(io, &receiver, &worker_trace, &worker_record))
        .map_err(|error| {
            Error::with_source(
                ErrorKind::Transport,
                "failed to start the card command worker",
                error,
            )
        })?;
    Ok(Card {
        sender,
        atr: Arc::new(Mutex::new(atr)),
        reader: record.id,
        backend: record.backend,
    })
}

enum CardJob {
    Transmit {
        command: Command,
        automatic: bool,
        complete: Completer<Response>,
    },
    Reset {
        complete: Completer<Atr>,
    },
    Begin {
        complete: Completer<()>,
    },
    End,
}

/// An exclusive, ordered command channel to one card.
pub struct Card {
    sender: Sender<CardJob>,
    atr: Arc<Mutex<Atr>>,
    reader: ReaderId,
    backend: BackendKind,
}

impl Card {
    /// Reader that owns this card connection.
    #[must_use]
    pub const fn reader_id(&self) -> ReaderId {
        self.reader
    }

    /// Backend used by this card connection.
    #[must_use]
    pub const fn backend(&self) -> BackendKind {
        self.backend
    }

    /// Most recent validated answer-to-reset.
    #[must_use]
    pub fn atr(&self) -> Atr {
        lock(&self.atr).clone()
    }

    /// Transmit a command, automatically handling `6Cxx` and chained `61xx` replies.
    pub fn transmit(&mut self, command: Command) -> Operation<'_, Response> {
        self.queue_transmit(command, true)
    }

    /// Transmit exactly one command and return exactly one response.
    pub fn transmit_raw(&mut self, command: Command) -> Operation<'_, Response> {
        self.queue_transmit(command, false)
    }

    fn queue_transmit(&mut self, command: Command, automatic: bool) -> Operation<'_, Response> {
        let (operation, complete) = Operation::pending();
        if self
            .sender
            .send(CardJob::Transmit {
                command,
                automatic,
                complete,
            })
            .is_err()
        {
            return Operation::ready(Err(card_worker_closed()));
        }
        operation
    }

    /// Reset the card and replace the cached ATR.
    pub fn reset(&mut self) -> Operation<'_, Atr> {
        let (operation, complete) = Operation::pending();
        if self.sender.send(CardJob::Reset { complete }).is_err() {
            return Operation::ready(Err(card_worker_closed()));
        }
        operation
    }

    /// Begin a transaction bracket tied to this mutable borrow.
    ///
    /// ```compile_fail
    /// fn cannot_interleave(card: &mut ccidkit::Card) {
    ///     let transaction = card.transaction().unwrap();
    ///     let _second = card.transmit(ccidkit::Command::new(0, 0x84, 0, 0));
    ///     drop(transaction);
    /// }
    /// ```
    pub fn transaction(&mut self) -> Result<Transaction<'_>> {
        let (operation, complete) = Operation::pending();
        self.sender
            .send(CardJob::Begin { complete })
            .map_err(|_| card_worker_closed())?;
        operation.wait()?;
        Ok(Transaction { card: self })
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Card")
            .field("reader", &self.reader)
            .field("backend", &self.backend)
            .field("atr", &self.atr())
            .finish_non_exhaustive()
    }
}

/// A transaction bracket that ends when dropped.
pub struct Transaction<'a> {
    card: &'a mut Card,
}

impl Transaction<'_> {
    /// Transmit with automatic `6Cxx`/`61xx` handling inside the transaction.
    pub fn transmit(&mut self, command: Command) -> Operation<'_, Response> {
        self.card.transmit(command)
    }

    /// Transmit exactly one APDU inside the transaction.
    pub fn transmit_raw(&mut self, command: Command) -> Operation<'_, Response> {
        self.card.transmit_raw(command)
    }

    /// Reset the card inside the transaction.
    pub fn reset(&mut self) -> Operation<'_, Atr> {
        self.card.reset()
    }

    /// Most recent ATR.
    #[must_use]
    pub fn atr(&self) -> Atr {
        self.card.atr()
    }
}

impl fmt::Debug for Transaction<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Transaction")
            .field("reader", &self.card.reader_id())
            .finish_non_exhaustive()
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        let _ignored = self.card.sender.send(CardJob::End);
    }
}

fn card_worker(
    mut io: CardIo,
    receiver: &Receiver<CardJob>,
    trace: &TraceHub,
    reader: &ReaderRecord,
) {
    while let Ok(job) = receiver.recv() {
        match job {
            CardJob::Transmit {
                command,
                automatic,
                complete,
            } => {
                let result = process_transmit(&command, automatic, trace, reader, |command| {
                    io.transmit_raw(command, || complete.is_cancelled())
                });
                complete.complete(result);
            },
            CardJob::Reset { complete } => complete.complete(io.reset()),
            CardJob::Begin { complete } => match io.transaction() {
                Ok(mut transaction) => {
                    complete.complete(Ok(()));
                    transaction_worker(&mut transaction, receiver, trace, reader);
                },
                Err(error) => complete.complete(Err(error)),
            },
            CardJob::End => {},
        }
    }
}

fn transaction_worker(
    transaction: &mut CardTransaction<'_>,
    receiver: &Receiver<CardJob>,
    trace: &TraceHub,
    reader: &ReaderRecord,
) {
    while let Ok(job) = receiver.recv() {
        match job {
            CardJob::Transmit {
                command,
                automatic,
                complete,
            } => {
                let result = process_transmit(&command, automatic, trace, reader, |command| {
                    transaction.transmit_raw(command, || complete.is_cancelled())
                });
                complete.complete(result);
            },
            CardJob::Reset { complete } => complete.complete(transaction.reset()),
            CardJob::Begin { complete } => {
                complete.complete(Err(Error::new(
                    ErrorKind::Busy,
                    "a transaction is already active on this card",
                )));
            },
            CardJob::End => return,
        }
    }
}

fn process_transmit<F>(
    command: &Command,
    automatic: bool,
    trace: &TraceHub,
    reader: &ReaderRecord,
    mut send: F,
) -> Result<Response>
where
    F: FnMut(&Command) -> Result<Response>,
{
    if automatic {
        transmit_automatic(command, trace, reader, &mut send)
    } else {
        exchange(command, trace, reader, &mut send)
    }
}

fn exchange<F>(
    command: &Command,
    trace: &TraceHub,
    reader: &ReaderRecord,
    send: &mut F,
) -> Result<Response>
where
    F: FnMut(&Command) -> Result<Response>,
{
    trace.emit(&TraceEvent::Frame(TraceFrame::new(
        Layer::Apdu,
        Direction::Out,
        reader.backend,
        reader.id,
        command.to_bytes(),
    )));
    let response = send(command)?;
    trace.emit(&TraceEvent::Frame(TraceFrame::new(
        Layer::Apdu,
        Direction::In,
        reader.backend,
        reader.id,
        response.to_bytes(),
    )));
    Ok(response)
}

fn transmit_automatic<F>(
    original: &Command,
    trace: &TraceHub,
    reader: &ReaderRecord,
    send: &mut F,
) -> Result<Response>
where
    F: FnMut(&Command) -> Result<Response>,
{
    let mut response = exchange(original, trace, reader, send)?;
    let [sw1, sw2] = response.status().bytes();
    if sw1 == 0x6C {
        let corrected = if sw2 == 0 { 256 } else { usize::from(sw2) };
        response = exchange(
            &original.with_replaced_expected_len(corrected)?,
            trace,
            reader,
            send,
        )?;
    }

    let mut data = response.data().to_vec();
    for _ in 0..32 {
        let [more, length] = response.status().bytes();
        if more != 0x61 {
            return Ok(Response::new(data, response.status()));
        }
        let expected = if length == 0 {
            256
        } else {
            usize::from(length)
        };
        let get_response =
            Command::new(original.class(), 0xC0, 0, 0).with_expected_len(expected)?;
        response = exchange(&get_response, trace, reader, send)?;
        data.extend_from_slice(response.data());
    }
    Err(Error::new(
        ErrorKind::Protocol,
        "card returned more than 32 chained GET RESPONSE statuses",
    ))
}

/// A change reported by a [`Monitor`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Event {
    /// A reader became visible.
    ReaderAdded(Reader),
    /// A reader disappeared.
    ReaderRemoved(ReaderId),
    /// A card was inserted.
    CardInserted(ReaderId),
    /// A card was removed.
    CardRemoved(ReaderId),
}

struct MonitorState {
    known: BTreeMap<ReaderId, ReaderRecord>,
}

/// A cancellation-safe stream of reader and card-presence changes.
pub struct Monitor {
    inner: Arc<ContextInner>,
    state: Arc<Mutex<MonitorState>>,
}

impl Monitor {
    /// Wait for the next observed change.
    pub fn next_event(&mut self) -> Operation<'_, Event> {
        let inner = Arc::clone(&self.inner);
        let state = Arc::clone(&self.state);
        spawn_operation("ccidkit-monitor", move |cancelled| {
            loop {
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                    return Err(Error::from_kind(ErrorKind::Cancelled));
                }

                if let Some(event) = inner.factory.virtual_event() {
                    let reader_id = {
                        let known = lock(&state);
                        known.known.keys().next().copied()
                    };
                    if let Some(reader_id) = reader_id {
                        return Ok(match event {
                            crate::backend::VirtualEvent::Inserted => {
                                Event::CardInserted(reader_id)
                            },
                            crate::backend::VirtualEvent::Removed => Event::CardRemoved(reader_id),
                        });
                    }
                }

                let current: BTreeMap<_, _> = inner
                    .factory
                    .readers()?
                    .into_iter()
                    .map(|record| (record.id, record))
                    .collect();
                {
                    let mut state = lock(&state);
                    if let Some(id) = state
                        .known
                        .keys()
                        .find(|id| !current.contains_key(id))
                        .copied()
                    {
                        state.known = current;
                        return Ok(Event::ReaderRemoved(id));
                    }
                    if let Some(record) = current
                        .values()
                        .find(|record| !state.known.contains_key(&record.id))
                        .cloned()
                    {
                        state.known = current;
                        return Ok(Event::ReaderAdded(Reader::new(Arc::clone(&inner), record)));
                    }
                    state.known = current;
                }
                thread::sleep(Duration::from_millis(100));
            }
        })
    }
}

impl fmt::Debug for Monitor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Monitor").finish_non_exhaustive()
    }
}

fn spawn_operation<T, F>(name: &str, work: F) -> Operation<'static, T>
where
    T: Send + 'static,
    F: FnOnce(&AtomicBool) -> Result<T> + Send + 'static,
{
    let (operation, complete) = Operation::pending();
    let fallback = complete.clone();
    let spawn = thread::Builder::new().name(name.to_owned()).spawn(move || {
        let result = work(complete.cancellation());
        complete.complete(result);
    });
    if let Err(error) = spawn {
        fallback.complete(Err(Error::with_source(
            ErrorKind::Transport,
            "failed to start an operation worker",
            error,
        )));
    }
    operation
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn card_worker_closed() -> Error {
    Error::new(ErrorKind::CardGone, "card command worker has stopped")
}

#[cfg(test)]
mod tests {
    use super::{Event, ReaderId};
    use crate::diagnostics::BackendKind;
    use crate::testing::{self, Scenario};
    use crate::{Atr, Command, Response, StatusWord};
    use std::time::Duration;

    fn atr() -> Atr {
        Atr::parse(&[0x3B, 0x00]).expect("valid fixture")
    }

    #[test]
    fn reader_ids_are_deterministic_and_backend_scoped() {
        let usb = ReaderId::from_name(BackendKind::NativeUsb, "reader");
        let pcsc = ReaderId::from_name(BackendKind::Pcsc, "reader");
        assert_eq!(usb, ReaderId::from_name(BackendKind::NativeUsb, "reader"));
        assert_ne!(usb, pcsc);
    }

    #[test]
    fn virtual_card_runs_the_public_quick_path() {
        let command = Command::new(0, 0x84, 0, 0);
        let expected = Response::new([1, 2, 3], StatusWord::from_u16(0x9000));
        let scenario = Scenario::new()
            .insert(atr())
            .respond(command.clone(), expected.clone());
        let context = testing::open(scenario).wait().expect("context");
        let mut card = context.open_first().wait().expect("card");
        let response = card.transmit(command).wait().expect("transmit");
        assert_eq!(response, expected);
    }

    #[test]
    fn monitor_reports_scripted_presence() {
        let context = testing::open(Scenario::new().insert(atr()))
            .wait()
            .expect("context");
        let mut monitor = context.monitor().expect("monitor");
        let event = monitor
            .next_event()
            .wait_timeout(Duration::from_secs(1))
            .expect("event");
        assert!(matches!(event, Event::CardInserted(_)));
    }

    #[test]
    fn automatic_transmit_corrects_le_and_collects_get_response() {
        let original = Command::new(0, 0xCA, 0, 0)
            .with_expected_len(1)
            .expect("Le");
        let corrected = Command::new(0, 0xCA, 0, 0)
            .with_expected_len(3)
            .expect("Le");
        let get_response = Command::new(0, 0xC0, 0, 0)
            .with_expected_len(2)
            .expect("Le");
        let scenario = Scenario::new()
            .insert(atr())
            .respond(
                original.clone(),
                Response::new([], StatusWord::from_u16(0x6C03)),
            )
            .respond(corrected, Response::new([1], StatusWord::from_u16(0x6102)))
            .respond(
                get_response,
                Response::new([2, 3], StatusWord::from_u16(0x9000)),
            );
        let context = testing::open(scenario).wait().expect("context");
        let mut card = context.open_first().wait().expect("card");
        let response = card.transmit(original).wait().expect("transmit");
        assert_eq!(response.data(), [1, 2, 3]);
        assert_eq!(response.status(), StatusWord::from_u16(0x9000));
    }

    #[test]
    fn raw_transmit_never_follows_card_status_policy() {
        let command = Command::new(0, 0xCA, 0, 0);
        let scenario = Scenario::new().insert(atr()).respond(
            command.clone(),
            Response::new([9], StatusWord::from_u16(0x6102)),
        );
        let context = testing::open(scenario).wait().expect("context");
        let mut card = context.open_first().wait().expect("card");
        let response = card.transmit_raw(command).wait().expect("transmit");
        assert_eq!(response.data(), [9]);
        assert_eq!(response.status(), StatusWord::from_u16(0x6102));
    }

    #[test]
    fn transaction_drop_releases_the_fifo_for_following_card_work() {
        let inside = Command::new(0, 1, 0, 0);
        let after = Command::new(0, 2, 0, 0);
        let scenario = Scenario::new()
            .insert(atr())
            .respond(
                inside.clone(),
                Response::new([1], StatusWord::from_u16(0x9000)),
            )
            .respond(
                after.clone(),
                Response::new([2], StatusWord::from_u16(0x9000)),
            );
        let context = testing::open(scenario).wait().expect("context");
        let mut card = context.open_first().wait().expect("card");
        {
            let mut transaction = card.transaction().expect("begin transaction");
            let response = transaction
                .transmit_raw(inside)
                .wait()
                .expect("transaction transmit");
            assert_eq!(response.data(), [1]);
        }
        let response = card
            .transmit_raw(after)
            .wait_timeout(Duration::from_secs(1))
            .expect("work after transaction");
        assert_eq!(response.data(), [2]);
    }

    #[test]
    fn dropping_a_hung_operation_unblocks_the_ordered_card_worker() {
        let first = Command::new(0, 1, 0, 0);
        let second = Command::new(0, 2, 0, 0);
        let scenario = Scenario::new().insert(atr()).hang().respond(
            second.clone(),
            Response::new([], StatusWord::from_u16(0x9000)),
        );
        let context = testing::open(scenario).wait().expect("context");
        let mut card = context.open_first().wait().expect("card");
        let operation = card.transmit(first);
        drop(operation);
        let response = card
            .transmit(second)
            .wait_timeout(Duration::from_secs(1))
            .expect("worker drains cancellation");
        assert!(response.status().is_success());
    }
}
