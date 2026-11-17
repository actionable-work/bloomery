//! Configuration policy types shared with the schema crate.

pub use bloomery_schema::specifications::*;

pub fn from_config(config: &crate::Config) -> Result<SpecificationPolicy, crate::Diagnostic> {
    SpecificationPolicy::from_value(
        config
            .checks
            .as_ref()
            .and_then(|checks| checks.get("specifications")),
    )
}

#[cfg(test)]
mod tests;
