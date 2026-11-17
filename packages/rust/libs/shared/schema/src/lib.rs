//! Configuration schema, specification policies, and portable diagnostics.

pub mod catalog;
pub mod diagnostics;
pub mod specifications;

pub use diagnostics::{Diagnostic, SourceLocation, render_diagnostics, sort_diagnostics};
