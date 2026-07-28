// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;

use qubit_fs::{
    FileSystem,
    FsError,
    FsErrorKind,
    FsOperation,
    FsUri,
};
use qubit_fs_registry::{
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemRegistryError,
    FileSystemResolution,
    FileSystemSpec,
};
use qubit_spi::error::ProviderFailure;
use qubit_spi::{
    ProviderDescriptor,
    ProviderId,
    ProviderMetadata,
    ProviderSelection,
    ServiceProvider,
};

#[test]
fn test_selection_conflict_is_contextual_without_a_source() {
    let requested = ProviderSelection::named("memory")
        .expect("requested selection is valid");
    let configured = ProviderSelection::named("local")
        .expect("configured selection is valid");
    let error = FileSystemRegistryError::SelectionConflict {
        requested,
        configured,
    };

    assert!(error.to_string().contains("conflicts"));
    assert!(error.source().is_none());
}

/// Verifies registry errors can join filesystem operation error flows.
#[test]
fn test_selection_conflict_converts_to_filesystem_error() {
    let requested = ProviderSelection::named("memory")
        .expect("requested selection is valid");
    let configured = ProviderSelection::named("local")
        .expect("configured selection is valid");
    let registry_error = FileSystemRegistryError::SelectionConflict {
        requested,
        configured,
    };

    let error: FsError = registry_error.into();

    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
    assert_eq!(FsOperation::Provider, error.operation());
    assert!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<FileSystemRegistryError>())
            .is_some(),
        "the filesystem error should retain the typed registry error",
    );
}

/// Verifies every SPI-backed registry error retains its immediate source.
#[test]
fn test_spi_backed_registry_errors_preserve_display_and_source() {
    let registration_registry = FileSystemRegistry::default();
    registration_registry
        .register(UnavailableProvider)
        .expect("initial provider registration should succeed");
    let registration = registration_registry
        .register(UnavailableProvider)
        .expect_err("duplicate registration should fail");
    assert_error_display_and_source(
        &registration,
        "provider registration failed",
    );

    let selection = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            FsUri::parse("invalid-:///resource").expect("URI should parse"),
        ))
        .expect_err("invalid URI scheme should fail selection construction");
    assert_error_display_and_source(
        &selection,
        "provider selection is invalid",
    );

    let resolution = FileSystemRegistry::default()
        .resolve_selected(
            &ProviderSelection::named("missing").expect("selection is valid"),
        )
        .expect_err("unknown provider selection should fail resolution");
    assert_error_display_and_source(&resolution, "provider resolution failed");

    let creation_registry = FileSystemRegistry::default();
    creation_registry
        .register(UnavailableProvider)
        .expect("provider registration should succeed");
    let creation = creation_registry
        .resolve_config(&FileSystemConfig::new(
            FsUri::parse("unavailable:///resource").expect("URI should parse"),
        ))
        .expect_err("unavailable provider should fail creation");
    assert_error_display_and_source(
        &creation,
        "filesystem provider creation failed",
    );
}

/// Verifies a creation aggregate preserves decisive filesystem diagnostics.
#[test]
fn test_creation_error_conversion_preserves_decisive_provider_context() {
    let registry = FileSystemRegistry::default();
    registry
        .register(UnavailableProvider)
        .expect("provider registration should succeed");
    let registry_error = registry
        .resolve_config(&FileSystemConfig::new(
            FsUri::parse("unavailable:///resource").expect("URI should parse"),
        ))
        .expect_err("unavailable provider should fail creation");

    let error: FsError = registry_error.into();

    assert_eq!(FsErrorKind::ProviderUnavailable, error.kind());
    assert_eq!(Some("unavailable"), error.provider());
    assert!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<FileSystemRegistryError>())
            .is_some(),
        "the filesystem error should retain the typed creation aggregate",
    );
}

/// Checks the common display and source contract for SPI-backed variants.
fn assert_error_display_and_source(
    error: &FileSystemRegistryError,
    message: &str,
) {
    assert!(error.to_string().contains(message));
    assert!(
        error.source().is_some(),
        "{message} should retain its source"
    );
}

struct UnavailableProvider;

impl ProviderMetadata for UnavailableProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("unavailable").expect("provider ID is valid"),
        )
    }
}

impl ServiceProvider<FileSystemSpec> for UnavailableProvider {
    fn create_configured(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderFailure<FsError>>
    {
        Err(ProviderFailure::unavailable(FsError::new(
            FsErrorKind::ProviderUnavailable,
            FsOperation::Provider,
            "provider is unavailable",
        )))
    }
}
