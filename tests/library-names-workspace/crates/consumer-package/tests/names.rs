#[test]
fn names_are_used() {
    assert_eq!(test_alias::value(), 7);
    assert_eq!(
        consumer_library::answer(),
        actual_library::value() + renamed_other::value()
    );
}
