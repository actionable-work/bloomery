//! Shared schema catalog plus model-owned health constraints.

pub use bloomery_schema::catalog::*;

pub fn validate_raw(raw: &toml::Value) -> Result<(), crate::Diagnostic> {
    bloomery_schema::catalog::validate_raw(raw)?;
    crate::health::HealthConfig::from_raw(Some(raw))?;
    Ok(())
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
