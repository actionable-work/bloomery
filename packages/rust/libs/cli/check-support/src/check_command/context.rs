use crate::output;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckError {
    pub code: &'static str,
    pub message: String,
}

impl CheckError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: "UsageError",
            message: message.into(),
        }
    }

    pub fn operational(message: impl Into<String>) -> Self {
        Self {
            code: "OperationalError",
            message: message.into(),
        }
    }

    pub fn retrieval(message: impl Into<String>) -> Self {
        Self {
            code: "RetrievalError",
            message: message.into(),
        }
    }
}

/// The subset of check-run context needed by retained health commands.
pub struct HealthContext<'a, Backend: ?Sized> {
    pub root: &'a Path,
    pub json_mode: bool,
    pub color: output::ColorMode,
    pub backend: &'a Backend,
    pub cache_base: Option<&'a Path>,
}
