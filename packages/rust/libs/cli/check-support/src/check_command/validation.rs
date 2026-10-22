use super::context::CheckError;
use super::model::valid_system_name;
use bloomery_cli_types::{CheckArgs, CheckOperation};

pub fn validate_args(args: &CheckArgs) -> Result<(), CheckError> {
    if args.operation.is_some()
        && (!args.selectors.is_empty()
            || !args.systems.is_empty()
            || args.jobs.is_some()
            || args.fail_fast)
    {
        return Err(CheckError::usage(
            "execution options --check, --system, --jobs, and --fail-fast are not accepted before retrieval subcommands",
        ));
    }
    if args.jobs == Some(0) {
        return Err(CheckError::usage("--jobs must be a positive integer"));
    }
    for selector in &args.selectors {
        if selector.is_empty() {
            return Err(CheckError::usage("--check selectors must not be empty"));
        }
    }
    for system in &args.systems {
        if !valid_system_name(system) {
            return Err(CheckError::usage(format!(
                "invalid Nix system name '{system}'"
            )));
        }
    }
    match &args.operation {
        Some(CheckOperation::List(list)) => {
            validate_page_values(list.offset, list.limit)?;
            if list.selectors.iter().any(String::is_empty) {
                return Err(CheckError::usage("--check selectors must not be empty"));
            }
            for system in &list.systems {
                if !valid_system_name(system) {
                    return Err(CheckError::usage(format!(
                        "invalid Nix system name '{system}'"
                    )));
                }
            }
        }
        Some(CheckOperation::Failures(failures)) => {
            validate_page_values(failures.offset, failures.limit)?;
        }
        Some(CheckOperation::Details(details)) => {
            validate_page_values(details.offset.unwrap_or(0), details.limit)?;
            if details.failure.is_empty() {
                return Err(CheckError::usage("failure ID must not be empty"));
            }
        }
        Some(CheckOperation::Health(health)) => {
            validate_page_values(health.offset, health.limit)?;
        }
        Some(CheckOperation::HealthUpdate(_)) | None => {}
    }
    Ok(())
}

fn validate_page_values(_offset: usize, limit: usize) -> Result<(), CheckError> {
    if limit == 0 {
        return Err(CheckError::usage("--limit must be a positive integer"));
    }
    Ok(())
}
