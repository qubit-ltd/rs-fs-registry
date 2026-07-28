use qubit_fs::Uri;
#[test]
fn test_resolution_boundary_uses_secret_free_uri() {
    assert!(Uri::parse("s3://bucket/key").is_ok());
    assert!(Uri::parse("s3://user:password@bucket/key").is_err());
}
