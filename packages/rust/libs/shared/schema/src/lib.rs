//! Configuration schema, specification policies, and portable diagnostics.

pub mod catalog;
pub mod diagnostics;
pub mod requirement_id;
pub mod specifications;

pub use diagnostics::{Diagnostic, SourceLocation, render_diagnostics, sort_diagnostics};
pub use requirement_id::{RequirementId, is_requirement_id, parse_requirement_id};
