#[test]
fn uses_the_public_aws_api() {
    use object_store::aws::AmazonS3ConfigKey;
    assert_eq!(
        aws_fixture::builder()
            .get_config_value(&AmazonS3ConfigKey::Region)
            .as_deref(),
        Some("us-east-1")
    );
}
