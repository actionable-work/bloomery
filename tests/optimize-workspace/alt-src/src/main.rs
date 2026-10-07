//! Alternate compilation source used to prove compilation invalidation.

fn accumulate(limit: u64) -> u64 {
    let mut total = 1u64;
    for value in 1..limit {
        total = total.wrapping_mul(value % 7 + 1);
    }
    total
}

fn main() {
    let total = accumulate(500);
    println!("opt-app alt result: {total}");
}
