mod command;
mod execution;

pub use bloomery_check_catalog::check_command::catalog;
pub use bloomery_check_health::check_command::health;
pub use bloomery_check_output::check_command::output;
pub use bloomery_check_retrieval::check_command::retrieval;
pub use bloomery_check_storage::check_command::store;
pub use bloomery_check_support::check_command::{interrupt, model, progress, scheduler};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use catalog::host_nix_system;
pub use command::{
    CheckArgs, GateOutcome, run, run_at, run_at_with_cache, run_at_with_color,
    run_at_with_color_and_mode, run_gate_at, run_gate_at_with_mode,
};
pub use model::RunStatus;
