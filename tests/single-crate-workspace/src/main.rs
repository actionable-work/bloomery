const COLOCATED_VAR: &str = env!("COLOCATED_OVERRIDE_VAR");
const TOP_LEVEL_VAR: &str = env!("TOP_LEVEL_OVERRIDE_VAR");

#[cfg(single_colocated_override)]
const COLOCATED_CFG_ACTIVE: bool = true;

#[cfg(not(single_colocated_override))]
const COLOCATED_CFG_ACTIVE: bool = false;

#[cfg(single_toplevel_override)]
const TOP_LEVEL_CFG_ACTIVE: bool = true;

#[cfg(not(single_toplevel_override))]
const TOP_LEVEL_CFG_ACTIVE: bool = false;

fn main() {
    println!(
        "single crate: {}/{}, cfgs: {}/{}",
        COLOCATED_VAR, TOP_LEVEL_VAR, COLOCATED_CFG_ACTIVE, TOP_LEVEL_CFG_ACTIVE
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_crate_overrides() {
        assert!(COLOCATED_CFG_ACTIVE);
        assert!(TOP_LEVEL_CFG_ACTIVE);
        assert_eq!(COLOCATED_VAR, "injected_from_single_member");
        assert_eq!(TOP_LEVEL_VAR, "injected_from_single_flake");
    }
}
