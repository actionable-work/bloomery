use bloomery_model::Diagnostic;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigErrorKind {
    /// Invalid invocation such as an unknown key or mistyped value.
    Usage,
    /// Read, validation, or write failure.
    Failure,
}

#[derive(Debug)]
pub struct ConfigError {
    kind: ConfigErrorKind,
    message: String,
    diagnostic: Option<Diagnostic>,
}

impl ConfigError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: ConfigErrorKind::Usage,
            message: message.into(),
            diagnostic: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            kind: ConfigErrorKind::Failure,
            message: message.into(),
            diagnostic: None,
        }
    }

    pub fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        Self {
            kind: ConfigErrorKind::Failure,
            message: diagnostic.message.clone(),
            diagnostic: Some(diagnostic),
        }
    }

    pub fn kind(&self) -> ConfigErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostic.as_ref()
    }

    pub fn exit_code(&self) -> u8 {
        match self.kind {
            ConfigErrorKind::Usage => 2,
            ConfigErrorKind::Failure => 1,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}
