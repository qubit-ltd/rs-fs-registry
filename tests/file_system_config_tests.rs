// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FsUri,
    UserMetadata,
};
use qubit_fs_registry::{
    CredentialRef,
    FileSystemConfig,
};
use qubit_spi::ProviderSelection;

#[test]
fn test_config_builder_preserves_validated_options_without_a_fallible_step() {
    let selection =
        ProviderSelection::named("mock").expect("selection should parse");
    let options = UserMetadata::new()
        .with("region", "test-1")
        .expect("metadata should accept a non-sensitive key");
    let config = FileSystemConfig::new(
        FsUri::parse("mock:///file.txt").expect("URI should parse"),
    )
    .with_selection(selection.clone())
    .with_options(options.clone());

    assert_eq!(Some(&selection), config.selection());
    assert_eq!(&options, config.options());
    assert!(config.credentials().is_none());
}

/// Verifies sensitive options fail before reaching the configuration builder.
#[test]
fn test_sensitive_options_are_rejected_while_building_user_metadata() {
    assert!(
        UserMetadata::new()
            .with("access_token", "plaintext")
            .is_err()
    );
}

/// Verifies configuration debugging exposes neither option values nor
/// credential reference contents.
#[test]
fn test_config_debug_redacts_values_and_credential_references() {
    let config = FileSystemConfig::new(
        FsUri::parse("mock:///resource").expect("URI should parse"),
    )
    .with_options(
        UserMetadata::new()
            .with("endpoint", "storage.internal")
            .expect("metadata should accept a non-sensitive key"),
    )
    .with_credentials(CredentialRef::Profile {
        name: "production".to_owned(),
    });

    let debug = format!("{config:?}");

    assert!(debug.contains("endpoint"));
    assert!(!debug.contains("storage.internal"));
    assert!(!debug.contains("production"));
}
