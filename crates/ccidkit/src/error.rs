// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

/// Stable categories callers may branch on without learning which backend failed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// An argument or encoded value is not valid.
    InvalidInput,
    /// No compatible reader is attached.
    NoReader,
    /// A reader exists but has no card present.
    CardAbsent,
    /// The card was removed while an operation was in progress.
    CardGone,
    /// Another process or transaction currently owns the resource.
    Busy,
    /// The process lacks permission to open the reader.
    PermissionDenied,
    /// The operation did not finish before its deadline.
    Timeout,
    /// A pending observation was cancelled.
    Cancelled,
    /// The selected backend is unavailable in this build or on this platform.
    BackendUnavailable,
    /// The transport failed without a more specific portable classification.
    Transport,
    /// Bytes received from a reader or card violate the active protocol.
    Protocol,
    /// The reader or backend does not implement the requested capability.
    NotSupported,
}

impl ErrorKind {
    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid input",
            Self::NoReader => "no compatible reader",
            Self::CardAbsent => "card absent",
            Self::CardGone => "card removed",
            Self::Busy => "resource busy",
            Self::PermissionDenied => "permission denied",
            Self::Timeout => "operation timed out",
            Self::Cancelled => "operation cancelled",
            Self::BackendUnavailable => "backend unavailable",
            Self::Transport => "transport failure",
            Self::Protocol => "protocol failure",
            Self::NotSupported => "operation not supported",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.description())
    }
}

/// A backend-neutral error with stable classification and useful diagnostic context.
#[derive(Clone, Debug)]
pub struct Error {
    kind: ErrorKind,
    message: Arc<str>,
    source: Option<Arc<dyn StdError + Send + Sync>>,
}

impl Error {
    /// Construct an error using the category's default message.
    #[must_use]
    pub fn from_kind(kind: ErrorKind) -> Self {
        Self::new(kind, kind.description())
    }

    /// Construct an error with portable, display-ready context.
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<Arc<str>>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(
        kind: ErrorKind,
        message: impl Into<Arc<str>>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(Arc::new(source)),
        }
    }

    /// Return the stable category intended for programmatic decisions.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Return the portable context rendered by [`Display`](fmt::Display).
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether retrying after a user or environment change can reasonably succeed.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::NoReader
                | ErrorKind::CardAbsent
                | ErrorKind::CardGone
                | ErrorKind::Busy
                | ErrorKind::PermissionDenied
                | ErrorKind::Timeout
                | ErrorKind::Cancelled
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn StdError + 'static))
    }
}

/// The result type returned by ccidkit operations.
pub type Result<T> = std::result::Result<T, Error>;
