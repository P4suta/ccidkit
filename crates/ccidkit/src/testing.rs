// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic readers for testing card applications without hardware.

use std::collections::VecDeque;

use crate::{Atr, Command, Context, ErrorKind, Operation, Response};

/// A consuming script for one virtual reader and card.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub(crate) name: String,
    pub(crate) atr: Option<Atr>,
    pub(crate) exchanges: VecDeque<ExchangeStep>,
    pub(crate) events: VecDeque<ScriptEvent>,
}

#[derive(Clone, Debug)]
pub(crate) enum ExchangeStep {
    Respond {
        command: Command,
        response: Response,
    },
    Fail(ErrorKind),
    Remove,
    Hang,
}

#[derive(Clone, Debug)]
pub(crate) enum ScriptEvent {
    Inserted,
    Removed,
}

impl Scenario {
    /// Start an empty scenario containing one named virtual reader and no card.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "ccidkit virtual reader".to_owned(),
            atr: None,
            exchanges: VecDeque::new(),
            events: VecDeque::new(),
        }
    }

    /// Set the reader name used by enumeration and diagnostics.
    #[must_use]
    pub fn reader(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Make a card with `atr` initially present and queue an insertion event.
    #[must_use]
    pub fn insert(mut self, atr: Atr) -> Self {
        self.atr = Some(atr);
        self.events.push_back(ScriptEvent::Inserted);
        self
    }

    /// Remove the card at the next exchange and queue a removal event.
    #[must_use]
    pub fn remove(mut self) -> Self {
        self.exchanges.push_back(ExchangeStep::Remove);
        self.events.push_back(ScriptEvent::Removed);
        self
    }

    /// Require `command` at the next exchange and return `response`.
    #[must_use]
    pub fn respond(mut self, command: Command, response: Response) -> Self {
        self.exchanges
            .push_back(ExchangeStep::Respond { command, response });
        self
    }

    /// Produce one portable error category at the next exchange.
    #[must_use]
    pub fn fail(mut self, kind: ErrorKind) -> Self {
        self.exchanges.push_back(ExchangeStep::Fail(kind));
        self
    }

    /// Make the next exchange wait until its [`Operation`] is dropped.
    #[must_use]
    pub fn hang(mut self) -> Self {
        self.exchanges.push_back(ExchangeStep::Hang);
        self
    }
}

impl Default for Scenario {
    fn default() -> Self {
        Self::new()
    }
}

/// Open an isolated context backed by `scenario`.
pub fn open(scenario: Scenario) -> Operation<'static, Context> {
    Context::from_scenario(scenario)
}
