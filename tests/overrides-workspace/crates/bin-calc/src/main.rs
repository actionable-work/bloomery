use lib_calc::compute_sum;
use lib_core::int_to_str;

const COLOCATED_VAR: &str = env!("COLOCATED_OVERRIDE_VAR");
const TOP_LEVEL_VAR: &str = env!("TOP_LEVEL_OVERRIDE_VAR");

#[cfg(bloomery_colocated_override)]
const COLOCATED_CFG_ACTIVE: bool = true;

#[cfg(not(bloomery_colocated_override))]
const COLOCATED_CFG_ACTIVE: bool = false;

#[cfg(bloomery_toplevel_override)]
const TOP_LEVEL_CFG_ACTIVE: bool = true;

#[cfg(not(bloomery_toplevel_override))]
const TOP_LEVEL_CFG_ACTIVE: bool = false;

fn main() {
    let result = compute_sum(15, 25);
    println!(
        "bin-calc result: {}, colocated: {}, top-level: {}, cfgs: {}/{}",
        int_to_str(result),
        COLOCATED_VAR,
        TOP_LEVEL_VAR,
        COLOCATED_CFG_ACTIVE,
        TOP_LEVEL_CFG_ACTIVE
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_calc() {
        assert_eq!(compute_sum(1, 2), 103);
    }

    #[test]
    fn test_colocated_override() {
        assert!(
            COLOCATED_CFG_ACTIVE,
            "Expected bloomery_colocated_override cfg flag from overrides.nix!"
        );
        assert_eq!(COLOCATED_VAR, "injected_from_member_override");
    }

    #[test]
    fn test_top_level_override() {
        assert!(
            TOP_LEVEL_CFG_ACTIVE,
            "Expected bloomery_toplevel_override cfg flag from flake.nix!"
        );
        assert_eq!(TOP_LEVEL_VAR, "injected_from_flake_nix");
    }
}
