use member_app::cfg_active;

#[test]
fn test_dir_integration() {
    assert!(cfg_active());
}
