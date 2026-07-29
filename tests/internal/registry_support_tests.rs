use qubit_fs::ConnectionUri;
use qubit_fs_registry::{
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemRegistryError,
};
#[test]
fn test_invalid_uri_scheme_is_rejected_without_default_fallback() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("invalid-:///resource").expect("URI should parse"),
    );
    let error = FileSystemRegistry::default()
        .resolve_config(&config)
        .expect_err("invalid scheme should not use default");
    assert!(matches!(error, FileSystemRegistryError::Selection(_)));
}
