//! Nix CLI conventions shared by the commands that drive Nix.

mod path_info;
mod store;
mod system;

pub use path_info::{PathEntry, PathInfo, parse_path_info};
pub use store::{
    eval_base_args, evaluated_derivation, full_store_path, is_store_path, parse_cannot_build,
    valid_derivation_path,
};
pub use system::{deduplicate_systems, host_nix_system, valid_system_name};
