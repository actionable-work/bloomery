//! Trivial binary used to exercise the PGO and BOLT optimization pipeline.

fn accumulate(limit: u64) -> u64 {
    let mut total = 0u64;
    for value in 0..limit {
        total = total.wrapping_add(value.wrapping_mul(value));
    }
    total
}

fn main() {
    let total = accumulate(1000) + opt_lib::offset();
    println!("opt-app result: {total}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_squares() {
        assert!(accumulate(1000) > 0);
    }
}
