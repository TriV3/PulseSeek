use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCategory {
    InvalidInput,
    NotFound,
    PermissionDenied,
    Unsupported,
    Unavailable,
    Conflict,
    Cancelled,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserErrorDescriptor {
    category: ErrorCategory,
    message: &'static str,
}

impl UserErrorDescriptor {
    pub const fn new(category: ErrorCategory, message: &'static str) -> Self {
        Self { category, message }
    }

    pub const fn category(&self) -> ErrorCategory {
        self.category
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }

    pub const fn for_category(category: ErrorCategory) -> Self {
        let message = match category {
            ErrorCategory::InvalidInput => "PulseSeek received invalid input.",
            ErrorCategory::NotFound => "PulseSeek could not find that item.",
            ErrorCategory::PermissionDenied => "PulseSeek could not access that item.",
            ErrorCategory::Unsupported => "PulseSeek does not support that operation.",
            ErrorCategory::Unavailable => "PulseSeek cannot complete that operation right now.",
            ErrorCategory::Conflict => "PulseSeek could not apply that change.",
            ErrorCategory::Cancelled => "The operation was cancelled.",
            ErrorCategory::Internal => "PulseSeek encountered an internal error.",
        };

        Self::new(category, message)
    }
}

impl fmt::Display for UserErrorDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    BrowserRead,
    AudioOutput,
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BrowserRead => "browser.read",
            Self::AudioOutput => "audio.output",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticContext {
    code: DiagnosticCode,
}

impl DiagnosticContext {
    pub const fn new(code: DiagnosticCode) -> Self {
        Self { code }
    }

    pub const fn code(&self) -> &'static str {
        self.code.as_str()
    }
}

impl fmt::Display for DiagnosticContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

pub struct ApplicationError {
    category: ErrorCategory,
    context: DiagnosticContext,
    source: Box<dyn Error + Send + Sync + 'static>,
}

impl ApplicationError {
    pub fn new<E>(category: ErrorCategory, context: DiagnosticContext, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self { category, context, source: Box::new(source) }
    }
}

impl fmt::Debug for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationError")
            .field("category", &self.category)
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.user_descriptor().fmt(formatter)
    }
}

impl Error for ApplicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl ErrorContract for ApplicationError {
    fn user_descriptor(&self) -> UserErrorDescriptor {
        UserErrorDescriptor::for_category(self.category)
    }

    fn diagnostic_context(&self) -> DiagnosticContext {
        self.context
    }
}

pub trait ErrorContract: Error {
    fn user_descriptor(&self) -> UserErrorDescriptor;

    fn diagnostic_context(&self) -> DiagnosticContext;
}
