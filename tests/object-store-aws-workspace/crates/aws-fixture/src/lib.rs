//! Compiles the public [`object_store::aws::AmazonS3Builder`] API.
//!
//! ```
//! use object_store::aws::AmazonS3ConfigKey;
//! let builder = aws_fixture::builder();
//! assert_eq!(builder.get_config_value(&AmazonS3ConfigKey::Bucket).as_deref(), Some("fixture-bucket"));
//! ```
use object_store::aws::AmazonS3Builder;

pub fn builder() -> AmazonS3Builder {
    AmazonS3Builder::new()
        .with_region("us-east-1")
        .with_bucket_name("fixture-bucket")
}

#[cfg(test)]
mod tests {
    #[test]
    fn configures_bucket_without_credentials() {
        use object_store::aws::AmazonS3ConfigKey;
        assert_eq!(
            super::builder()
                .get_config_value(&AmazonS3ConfigKey::Bucket)
                .as_deref(),
            Some("fixture-bucket")
        );
    }
}
