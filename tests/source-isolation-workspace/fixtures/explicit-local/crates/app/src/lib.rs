pub const EMBEDDED: &str = include_str!("embedded.md");
pub const DATA: &[u8] = include_bytes!("data.bin");

pub fn message() -> &'static str {
    dep::message()
}
