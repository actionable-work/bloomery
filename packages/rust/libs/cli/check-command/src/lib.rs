mod check_command;

pub mod output {
    pub use bloomery_cli_output::*;
}

pub use check_command::output as check_output;
pub use check_command::{
    CheckArgs, GateOutcome, RunStatus, host_nix_system, run, run_at, run_at_with_cache,
    run_at_with_color, run_at_with_color_and_mode, run_gate_at, run_gate_at_with_mode,
};
