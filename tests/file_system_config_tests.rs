// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::ConnectionUri;
use qubit_fs::FsErrorKind;
use qubit_fs::NonSensitiveMetadata;
use qubit_fs::UserMetadata;
use qubit_fs_registry::CredentialRef;
use qubit_fs_registry::FileSystemConfig;
use qubit_spi::ProviderSelection;

/// Builder methods accept already validated metadata without a fallible step.
#[test]
fn test_config_builder_preserves_validated_options_without_a_fallible_step() {
    let selection = ProviderSelection::named("mock").expect("selection should parse");
    let options = UserMetadata::new()
        .with("region", "test-1")
        .expect("metadata should accept a non-sensitive key");
    let options = NonSensitiveMetadata::from(options);
    let config =
        FileSystemConfig::new(ConnectionUri::parse("mock:///file.txt").expect("URI should parse"))
            .with_selection(selection.clone())
            .with_options(options.clone());

    assert_eq!(Some(&selection), config.selection());
    assert_eq!(&options, config.options());
    assert!(config.credential().is_none());
}

/// Builder methods retain every public configuration component for provider
/// factories without exposing sensitive values.
#[test]
fn test_config_builder_exposes_metadata_uri_and_credential_reference() {
    let uri = ConnectionUri::parse("mock:///metadata").expect("URI should parse");
    let metadata = NonSensitiveMetadata::from(
        UserMetadata::new()
            .with("zone", "test-zone")
            .expect("metadata should accept a non-sensitive key"),
    );
    let config = FileSystemConfig::new(uri.clone())
        .with_metadata(metadata.clone())
        .with_credential(CredentialRef::DefaultChain);

    assert_eq!(config.uri(), &uri);
    assert_eq!(config.metadata(), &metadata);
    assert_eq!(config.credential(), Some(&CredentialRef::DefaultChain));
}

/// Verifies sensitive options fail before reaching the configuration builder.
#[test]
fn test_sensitive_options_are_rejected_while_building_user_metadata() {
    let error = UserMetadata::new()
        .with("access_token", "secret")
        .expect_err("credential-like key must be rejected");

    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
}

/// Verifies configuration debugging exposes neither option values nor
/// credential reference contents.
#[test]
fn test_config_debug_redacts_values_and_credential_references() {
    let config =
        FileSystemConfig::new(ConnectionUri::parse("mock:///resource").expect("URI should parse"))
            .with_options(NonSensitiveMetadata::from(
                UserMetadata::new()
                    .with("endpoint", "storage.internal")
                    .expect("metadata should accept a non-sensitive key"),
            ))
            .with_credential(CredentialRef::Profile {
                name: "production".to_owned(),
            });

    let debug = format!("{config:?}");

    assert!(debug.contains("endpoint"));
    assert!(!debug.contains("storage.internal"));
    assert!(!debug.contains("production"));
}

/// Verifies ordinary formatting delegates URI secret masking to
/// `ConnectionUri`.
#[test]
fn test_config_display_and_debug_never_expose_connection_secret() {
    let config = FileSystemConfig::new(
        ConnectionUri::parse("s3://user:password@bucket/key?token=secret")
            .expect("connection URI should parse"),
    );

    for rendered in [format!("{config}"), format!("{config:?}")] {
        assert!(!rendered.contains("password"));
        assert!(!rendered.contains("secret"));
    }
}
