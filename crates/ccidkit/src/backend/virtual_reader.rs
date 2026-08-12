// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::backend::ReaderRecord;
use crate::diagnostics::{BackendKind, Capabilities, ExchangeLevel};
use crate::testing::{ExchangeStep, Scenario, ScriptEvent};
use crate::{Atr, Command, Error, ErrorKind, ReaderId, Response, Result};

#[derive(Debug)]
pub(crate) struct VirtualState {
    scenario: Scenario,
    present: bool,
}

impl VirtualState {
    pub(crate) fn new(scenario: Scenario) -> Self {
        let present = scenario.atr.is_some();
        Self { scenario, present }
    }
}

fn lock(state: &Arc<Mutex<VirtualState>>) -> MutexGuard<'_, VirtualState> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) fn readers(state: &Arc<Mutex<VirtualState>>) -> Result<Vec<ReaderRecord>> {
    let state = lock(state);
    if state.scenario.name.trim().is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "virtual reader name cannot be empty",
        ));
    }
    let name: Arc<str> = Arc::from(state.scenario.name.as_str());
    Ok(vec![ReaderRecord {
        id: ReaderId::from_name(BackendKind::Virtual, &name),
        name: Arc::clone(&name),
        backend: BackendKind::Virtual,
        capabilities: capabilities(),
        selector: name,
    }])
}

const fn capabilities() -> Capabilities {
    Capabilities::new(1, 65_538, ExchangeLevel::ExtendedApdu, true, true)
}

pub(crate) fn connect(state: &Arc<Mutex<VirtualState>>) -> Result<(VirtualCard, Atr)> {
    let atr = {
        let state_guard = lock(state);
        if !state_guard.present {
            return Err(Error::from_kind(ErrorKind::CardAbsent));
        }
        state_guard
            .scenario
            .atr
            .clone()
            .ok_or_else(|| Error::from_kind(ErrorKind::CardAbsent))?
    };
    Ok((
        VirtualCard {
            state: Arc::clone(state),
        },
        atr,
    ))
}

#[derive(Debug)]
pub(crate) struct VirtualCard {
    state: Arc<Mutex<VirtualState>>,
}

impl VirtualCard {
    pub(crate) fn transmit(
        &mut self,
        command: &Command,
        cancelled: impl Fn() -> bool,
    ) -> Result<Response> {
        let step = {
            let mut state = lock(&self.state);
            if !state.present {
                return Err(Error::from_kind(ErrorKind::CardGone));
            }
            state.scenario.exchanges.pop_front()
        };
        match step {
            Some(ExchangeStep::Respond {
                command: expected,
                response,
            }) => {
                if expected == *command {
                    Ok(response)
                } else {
                    Err(Error::new(
                        ErrorKind::Protocol,
                        format!(
                            "virtual scenario expected {:02X?}, received {:02X?}",
                            expected.to_bytes(),
                            command.to_bytes()
                        ),
                    ))
                }
            },
            Some(ExchangeStep::Fail(kind)) => Err(Error::from_kind(kind)),
            Some(ExchangeStep::Remove) => {
                lock(&self.state).present = false;
                Err(Error::from_kind(ErrorKind::CardGone))
            },
            Some(ExchangeStep::Hang) => loop {
                if cancelled() {
                    return Err(Error::from_kind(ErrorKind::Cancelled));
                }
                std::thread::sleep(Duration::from_millis(10));
            },
            None => Err(Error::new(
                ErrorKind::Protocol,
                "virtual scenario has no exchange remaining",
            )),
        }
    }

    pub(crate) fn reset(&mut self) -> Result<Atr> {
        let state = lock(&self.state);
        if !state.present {
            return Err(Error::from_kind(ErrorKind::CardGone));
        }
        state
            .scenario
            .atr
            .clone()
            .ok_or_else(|| Error::from_kind(ErrorKind::CardGone))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum VirtualEvent {
    Inserted,
    Removed,
}

pub(crate) fn next_event(state: &Arc<Mutex<VirtualState>>) -> Option<VirtualEvent> {
    let mut state = lock(state);
    state.scenario.events.pop_front().map(|event| match event {
        ScriptEvent::Inserted => VirtualEvent::Inserted,
        ScriptEvent::Removed => VirtualEvent::Removed,
    })
}
