#[test]
fn fixture_content_is_available() {
    let fixture = include_str!("fixture.txt");
    assert!(fixture.contains("fixture"));
}
