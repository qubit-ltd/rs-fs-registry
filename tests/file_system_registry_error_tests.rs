// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
};
use qubit_fs_registry::FileSystemRegistryError;
use qubit_spi::ProviderSelection;
use std::error::Error;

use crate::file_system_registry_tests::FailingProvider;
use qubit_fs::ConnectionUri;
use qubit_fs_registry::{
    FileSystemConfig,
    FileSystemRegistry,
};
/// Selection conflicts convert to filesystem invalid-options errors.
#[test]
fn test_selection_conflict_converts_to_invalid_options() {
    let error = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("requested")
            .expect("valid selector"),
        configured: ProviderSelection::named("configured")
            .expect("valid selector"),
    };
    assert!(error.source().is_none());
    let fs_error: FsError = error.into();
    assert_eq!(fs_error.kind(), FsErrorKind::InvalidOptions);
    assert_eq!(fs_error.operation(), FsOperation::Provider);
}
/// Invalid registry configurations do not expose an underlying source error.
#[test]
fn test_invalid_configuration_never_has_a_source() {
    let error = FileSystemRegistryError::InvalidConfiguration {
        message: "embedded and referenced credentials conflict",
    };
    assert!(
        error
            .to_string()
            .contains("invalid filesystem configuration")
    );
    assert!(error.source().is_none());
    let fs_error: FsError = error.into();
    assert_eq!(fs_error.kind(), FsErrorKind::InvalidOptions);
    assert_eq!(fs_error.operation(), FsOperation::Provider);
}

/// Registry error formatting redacts provider and selection payloads.
#[test]
fn test_error_display_and_debug_do_not_expose_provider_or_selection_payloads() {
    let error = FileSystemRegistryError::SelectionConflict {
        requested: ProviderSelection::named("production-secret-provider")
            .expect("valid selector"),
        configured: ProviderSelection::named("other-secret-provider")
            .expect("valid selector"),
    };
    for rendered in [format!("{error}"), format!("{error:?}")] {
        assert!(!rendered.contains("production-secret-provider"));
        assert!(!rendered.contains("other-secret-provider"));
    }
}

/// Registration, resolution, and selection failures retain sources while
/// converting to provider-neutral filesystem errors.
#[test]
fn test_registry_error_variants_format_and_convert_safely() {
    let registration = {
        let registry = FileSystemRegistry::default();
        registry
            .register(FailingProvider::new("duplicate"))
            .expect("register first provider");
        registry
            .register(FailingProvider::new("duplicate"))
            .expect_err("duplicate provider must fail")
    };
    let resolution = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            ConnectionUri::parse("missing:///resource")
                .expect("URI should parse"),
        ))
        .expect_err("missing provider must fail resolution");
    let selection = FileSystemRegistry::default()
        .resolve_config(&FileSystemConfig::new(
            ConnectionUri::parse("invalid-:///resource")
                .expect("URI should parse"),
        ))
        .expect_err("invalid scheme must fail selection");
    let creation = {
        let registry = FileSystemRegistry::default();
        registry
            .register(FailingProvider::new("creation"))
            .expect("register provider");
        registry
            .resolve_config(&FileSystemConfig::new(
                ConnectionUri::parse("creation:///resource")
                    .expect("URI should parse"),
            ))
            .expect_err("unavailable provider must fail creation")
    };

    for (error, expected_kind, expected_provider) in [
        (registration, FsErrorKind::Conflict, None),
        (resolution, FsErrorKind::ProviderUnavailable, None),
        (selection, FsErrorKind::InvalidUri, None),
        (creation, FsErrorKind::ProviderUnavailable, Some("creation")),
    ] {
        assert!(error.source().is_some());
        assert!(!format!("{error}").is_empty());
        assert!(!format!("{error:?}").is_empty());
        let fs_error: FsError = error.into();
        assert_eq!(fs_error.kind(), expected_kind);
        assert_eq!(fs_error.operation(), FsOperation::Provider);
        assert_eq!(fs_error.provider(), expected_provider);
        assert!(std::error::Error::source(&fs_error).is_some());
    }
}
