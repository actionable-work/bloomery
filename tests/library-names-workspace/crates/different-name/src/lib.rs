pub fn value() -> u32 {
    #[cfg(feature = "extra")]
    assert_eq!(optional_alias::value(), 22);
    20
}
