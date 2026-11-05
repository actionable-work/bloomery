fn main() {
    assert_eq!(
        consumer_library::answer(),
        actual_library::value() + renamed_other::value()
    );
    println!("42");
}
