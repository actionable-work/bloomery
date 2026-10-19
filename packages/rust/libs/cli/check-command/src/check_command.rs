mod catalog;
mod command;
mod details;
mod execution;
mod health;
mod interrupt;
mod model;
mod nix_progress;
pub mod output;
mod progress;
mod retrieval;
mod scheduler;
mod store;

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
