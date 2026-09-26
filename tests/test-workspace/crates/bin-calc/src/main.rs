use lib_core::int_to_str;
use lib_calc::compute_sum;

fn main() {
    let result = compute_sum(15, 25);
    println!("bin-calc result: {}", int_to_str(result));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_calc() {
        assert_eq!(compute_sum(1, 2), 103);
    }
}
