use lib_core::int_to_str;

pub fn format_message(tag: &str, count: i64) -> String {
    let count_str = int_to_str(count);
    format!("{}: {} items", tag, count_str)
}

pub fn banner() -> &'static str {
    env!("MSG_BANNER")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg() {
        assert_eq!(format_message("Orders", 7), "Orders: 7 items");
        assert_eq!(banner(), "[Bloomery System]");
    }
}
