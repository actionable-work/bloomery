fn main() {
    use object_store::aws::AmazonS3ConfigKey;
    assert_eq!(
        aws_fixture::builder()
            .get_config_value(&AmazonS3ConfigKey::Bucket)
            .as_deref(),
        Some("fixture-bucket")
    );
    println!("AWS fixture passed");
}
