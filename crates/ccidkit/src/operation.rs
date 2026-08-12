// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use crate::{Error, ErrorKind, Result};

struct State<T> {
    value: Option<Result<T>>,
    waker: Option<Waker>,
}

struct Completion<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
    cancelled: AtomicBool,
}

impl<T> Completion<T> {
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// The producer half of an [`Operation`].
pub(crate) struct Completer<T> {
    completion: Arc<Completion<T>>,
}

impl<T> Clone for Completer<T> {
    fn clone(&self) -> Self {
        Self {
            completion: Arc::clone(&self.completion),
        }
    }
}

impl<T> Completer<T> {
    /// Finish the operation. A result is accepted at most once.
    pub(crate) fn complete(self, value: Result<T>) {
        let wake = {
            let mut state = self.completion.lock();
            if state.value.is_some() {
                return;
            }
            state.value = Some(value);
            state.waker.take()
        };
        self.completion.ready.notify_all();
        if let Some(waker) = wake {
            waker.wake();
        }
    }

    /// Whether the consumer dropped or timed out before completion.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.completion.cancelled.load(Ordering::Acquire)
    }

    /// Shared cancellation flag for a worker whose closure must outlive this borrow.
    pub(crate) fn cancellation(&self) -> &AtomicBool {
        &self.completion.cancelled
    }
}

/// One runtime-neutral I/O operation.
///
/// Await the value in asynchronous code, or use [`wait`](Self::wait) in synchronous
/// code. Dropping an observation operation requests cancellation. Card commands already
/// dispatched to a worker are drained before that card accepts its next command.
#[derive(Debug)]
#[must_use = "operations do nothing useful unless awaited or waited"]
pub struct Operation<'a, T> {
    completion: Arc<Completion<T>>,
    finished: bool,
    borrow: PhantomData<&'a mut ()>,
}

impl<T> Operation<'_, T> {
    pub(crate) fn pending() -> (Self, Completer<T>) {
        let completion = Arc::new(Completion {
            state: Mutex::new(State {
                value: None,
                waker: None,
            }),
            ready: Condvar::new(),
            cancelled: AtomicBool::new(false),
        });
        (
            Self {
                completion: Arc::clone(&completion),
                finished: false,
                borrow: PhantomData,
            },
            Completer { completion },
        )
    }

    pub(crate) fn ready(value: Result<T>) -> Self {
        let (operation, completer) = Self::pending();
        completer.complete(value);
        operation
    }

    fn take_ready(&mut self) -> Option<Result<T>> {
        let value = self.completion.lock().value.take();
        if value.is_some() {
            self.finished = true;
        }
        value
    }

    /// Block the current thread until the operation completes.
    pub fn wait(mut self) -> Result<T> {
        loop {
            if let Some(value) = self.take_ready() {
                return value;
            }
            let guard = self.completion.lock();
            if guard.value.is_some() {
                drop(guard);
                continue;
            }
            match self.completion.ready.wait(guard) {
                Ok(waited) => drop(waited),
                Err(poisoned) => drop(poisoned.into_inner()),
            }
        }
    }

    /// Block for at most `timeout` and request cancellation if it elapses.
    pub fn wait_timeout(mut self, timeout: Duration) -> Result<T> {
        let Some(deadline) = Instant::now().checked_add(timeout) else {
            return self.wait();
        };
        loop {
            if let Some(value) = self.take_ready() {
                return value;
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(Error::new(
                    ErrorKind::Timeout,
                    "operation did not finish before the caller's deadline",
                ));
            }
            let remaining = deadline.saturating_duration_since(now);
            let guard = self.completion.lock();
            if guard.value.is_some() {
                drop(guard);
                continue;
            }
            match self.completion.ready.wait_timeout(guard, remaining) {
                Ok((waited, _)) => drop(waited),
                Err(poisoned) => {
                    let (waited, _) = poisoned.into_inner();
                    drop(waited);
                },
            }
        }
    }
}

impl<T> Future for Operation<'_, T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let operation = self.get_mut();
        if let Some(value) = operation.take_ready() {
            return Poll::Ready(value);
        }

        let mut state = operation.completion.lock();
        if let Some(value) = state.value.take() {
            operation.finished = true;
            return Poll::Ready(value);
        }
        if state
            .waker
            .as_ref()
            .is_none_or(|registered| !registered.will_wake(context.waker()))
        {
            state.waker = Some(context.waker().clone());
        }
        Poll::Pending
    }
}

impl<T> Drop for Operation<'_, T> {
    fn drop(&mut self) {
        if !self.finished {
            self.completion.cancelled.store(true, Ordering::Release);
            self.completion.ready.notify_all();
        }
    }
}

impl<T> std::fmt::Debug for Completion<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Completion")
            .field("cancelled", &self.cancelled.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::Operation;
    use std::time::Duration;

    #[test]
    fn ready_operation_waits_without_a_runtime() {
        let value = Operation::ready(Ok::<_, crate::Error>(7_u8)).wait();
        assert!(matches!(value, Ok(7)));
    }

    #[test]
    fn timeout_marks_the_producer_cancelled() {
        let (operation, completer) = Operation::<()>::pending();
        let result = operation.wait_timeout(Duration::ZERO);
        assert!(matches!(result, Err(error) if error.kind() == crate::ErrorKind::Timeout));
        assert!(completer.is_cancelled());
    }
}
