use lib_calc::compute_product;
use lib_msg::{banner, format_message};

fn main() {
    let total = compute_product(7, 8);
    let msg = format_message("Calculated Total", total);
    println!("{} -> {}", banner(), msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_report() {
        let total = compute_product(2, 3);
        assert_eq!(format_message("Test", total), "Test: 6 items");
    }
}
