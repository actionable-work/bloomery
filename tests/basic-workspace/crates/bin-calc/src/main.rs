use lib_calc::compute_sum;
use lib_core::int_to_str;

fn main() {
    let result = compute_sum(15, 25);
    println!(
        "bin-calc result: {}, override: {}",
        int_to_str(result),
        COLOCATED_OVERRIDE_ACTIVE
    );
}

#[cfg(bloomery_colocated_override)]
const COLOCATED_OVERRIDE_ACTIVE: bool = true;

#[cfg(not(bloomery_colocated_override))]
const COLOCATED_OVERRIDE_ACTIVE: bool = false;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_calc() {
        assert_eq!(compute_sum(1, 2), 103);
    }

    #[test]
    fn test_colocated_override_active() {
        assert!(
            COLOCATED_OVERRIDE_ACTIVE,
            "Expected bloomery_colocated_override cfg flag from overrides.nix!"
        );
    }
}
