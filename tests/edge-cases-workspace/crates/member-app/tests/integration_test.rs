use member_app::cfg_active;

#[test]
fn test_integration_and_cfg() {
    assert!(
        cfg_active(),
        "integration test can import crate library and sees build cfg"
    );
}
